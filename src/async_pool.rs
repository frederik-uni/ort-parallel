use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, SystemTimeError},
};

use ort::session::{
    Input, RunOptions, SelectedOutputMarker, Session, SessionInputs, SessionOutputs,
    builder::{PrepackedWeights, SessionBuilder},
};

use tokio::sync::{Mutex, Semaphore};

use crate::SessionBuilderFactory;

pub struct AsyncSessionPool {
    sessions: Arc<Mutex<Vec<Arc<Mutex<Session>>>>>,
    available_sessions: Arc<Mutex<Vec<usize>>>,
    sem: Arc<Semaphore>,
    max: usize,
    builder: SessionBuilderFactory,
    file: PathBuf,
    pub inputs: Vec<Input>,
}

fn clone_inputs(inp: &Input) -> Input {
    Input {
        name: inp.name.clone(),
        input_type: inp.input_type.clone(),
    }
}

impl AsyncSessionPool {
    pub fn commit_from_file(
        builder: SessionBuilder,
        path: &Path,
        max_sessions: usize,
    ) -> ort::Result<Self> {
        assert!(max_sessions > 0);
        let prepacked_weights = PrepackedWeights::new();
        let builder = builder.with_prepacked_weights(&prepacked_weights)?;
        let fs = builder.clone().commit_from_file(path)?;
        let inputs = fs.inputs.iter().map(clone_inputs).collect::<Vec<Input>>();
        Ok(Self {
            inputs,
            sessions: Arc::new(Mutex::new(vec![Arc::new(Mutex::new(fs))])),
            sem: Arc::new(Semaphore::new(max_sessions)),
            available_sessions: Arc::new(Mutex::new(vec![0])),
            max: max_sessions,
            builder: SessionBuilderFactory(builder),
            file: path.to_path_buf(),
        })
    }

    pub async fn load_all(&self) -> ort::Result<()> {
        let count = self.max - self.sessions.lock().await.len();
        let mut sessions = self.sessions.lock().await;
        let mut avail = self.available_sessions.lock().await;
        if count != 0 {
            for _ in 0..count {
                let session = self.create_new()?;
                sessions.push(session.clone());
                avail.push(sessions.len() - 1);
            }
        }
        Ok(())
    }

    fn create_new(&self) -> Result<Arc<Mutex<Session>>, ort::Error> {
        Ok(Arc::new(Mutex::new(
            self.builder.generate().commit_from_file(&self.file)?,
        )))
    }

    async fn release_session(&self, idx: usize) {
        self.available_sessions.lock().await.push(idx);
        self.sem.add_permits(1);
    }

    async fn get_session(&self) -> Result<(Arc<Mutex<Session>>, usize), ort::Error> {
        let _permit = self.sem.acquire().await.unwrap();

        if let Some(idx) = self.available_sessions.lock().await.pop() {
            let sessions = self.sessions.lock().await;
            return Ok((sessions[idx].clone(), idx));
        }

        if self.sessions.lock().await.len() < self.max {
            let session = match self.create_new() {
                Ok(v) => v,
                Err(e) => {
                    self.sem.add_permits(1);
                    return Err(e);
                }
            };
            let mut sessions = self.sessions.lock().await;
            sessions.push(session.clone());
            return Ok((session, sessions.len() - 1));
        }

        unreachable!()
    }

    pub async fn run_async<'r, 's: 'r, 'i, 'v: 'i + 'r, O: SelectedOutputMarker, const N: usize>(
        &'s self,
        input_values: impl Into<SessionInputs<'i, 'v, N>>,
        run_options: &'r RunOptions<O>,
    ) -> Result<SessionOutputs<'r>, ort::Error> {
        let (ses, idx) = self.get_session().await?;
        let ses: &'s mut Session = unsafe { &mut *(&mut *ses.lock().await as *mut Session) };
        let out = match ses.run_async(input_values, run_options) {
            Ok(v) => v.await,
            Err(e) => Err(e),
        };
        self.release_session(idx).await;
        out
    }

    pub async fn run_async_profiled<
        'r,
        's: 'r,
        'i,
        'v: 'i + 'r,
        O: SelectedOutputMarker,
        const N: usize,
    >(
        &'s self,
        input_values: impl Into<SessionInputs<'i, 'v, N>>,
        run_options: &'r RunOptions<O>,
    ) -> Result<
        (
            SessionOutputs<'r>,
            Result<Duration, SystemTimeError>,
            String,
        ),
        ort::Error,
    > {
        let (ses, idx) = self.get_session().await?;
        let ses_ptr = &mut *ses.lock().await as *mut Session;
        let ses1: &'s mut Session = unsafe { &mut *(ses_ptr) };
        let ses2: &'s mut Session = unsafe { &mut *(ses_ptr) };

        let earlier = SystemTime::now();
        let _ = ses1.profiling_start_ns()?;
        let out = match ses1.run_async(input_values, run_options) {
            Ok(v) => v.await,
            Err(e) => Err(e),
        };
        let end = ses2.end_profiling()?;
        let now = SystemTime::now().duration_since(earlier);
        self.release_session(idx).await;
        out.map(|v| (v, now, end))
    }
}
