use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use ort::{
    io_binding::IoBinding,
    session::{
        NoSelectedOutputs, RunOptions, SelectedOutputMarker, Session, SessionInputs,
        SessionOutputs,
        builder::{PrepackedWeights, SessionBuilder},
    },
};
use parking_lot::Mutex;

use crate::{SessionBuilderFactory, semaphore::Semaphore};

pub struct SessionPool {
    sessions: Arc<Mutex<Vec<Arc<Mutex<Session>>>>>,
    available_sessions: Arc<Mutex<Vec<usize>>>,
    sem: Arc<Semaphore>,
    max: usize,
    builder: SessionBuilderFactory,
    file: PathBuf,
}

impl SessionPool {
    pub fn commit_from_file(
        builder: SessionBuilder,
        path: &Path,
        max_sessions: usize,
    ) -> ort::Result<Self> {
        assert!(max_sessions > 0);
        let prepacked_weights = PrepackedWeights::new();
        let builder = builder.with_prepacked_weights(&prepacked_weights)?;
        Ok(Self {
            sessions: Arc::new(Mutex::new(vec![Arc::new(Mutex::new(
                builder.clone().commit_from_file(path)?,
            ))])),
            sem: Arc::new(Semaphore::new(max_sessions)),
            available_sessions: Arc::new(Mutex::new(vec![0])),
            max: max_sessions,
            builder: SessionBuilderFactory(builder),
            file: path.to_path_buf(),
        })
    }

    pub fn load_all(&self) -> ort::Result<()> {
        let count = self.max - self.sessions.lock().len();
        let mut sessions = self.sessions.lock();
        let mut avail = self.available_sessions.lock();
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

    fn release_session(&self, idx: usize) {
        self.available_sessions.lock().push(idx);
        self.sem.release();
    }

    fn get_session(&self) -> Result<(Arc<Mutex<Session>>, usize), ort::Error> {
        let _permit = self.sem.acquire();

        if let Some(idx) = self.available_sessions.lock().pop() {
            let sessions = self.sessions.lock();
            return Ok((sessions[idx].clone(), idx));
        }

        if self.sessions.lock().len() < self.max {
            let session = match self.create_new() {
                Ok(v) => v,
                Err(e) => {
                    self.sem.release();
                    return Err(e);
                }
            };
            let mut sessions = self.sessions.lock();
            sessions.push(session.clone());
            return Ok((session, sessions.len() - 1));
        }

        unreachable!()
    }

    pub fn run_binding<'b, 's: 'b>(
        &'s self,
        binding: &'b IoBinding,
    ) -> ort::Result<SessionOutputs<'b>> {
        let (ses, idx) = self.get_session()?;
        let ses: &'s mut Session = unsafe { &mut *(&mut *ses.lock() as *mut Session) };
        let out = ses.run_binding(binding);
        self.release_session(idx);
        out
    }

    pub fn run_binding_with_options<'r, 'b, 's: 'b>(
        &'s self,
        binding: &'b IoBinding,
        run_options: &'r RunOptions<NoSelectedOutputs>,
    ) -> ort::Result<SessionOutputs<'b>> {
        let (ses, idx) = self.get_session()?;
        let ses: &'s mut Session = unsafe { &mut *(&mut *ses.lock() as *mut Session) };
        let out = ses.run_binding_with_options(binding, run_options);
        self.release_session(idx);
        out
    }

    pub fn run<'s, 'i, 'v: 'i, const N: usize>(
        &'s self,
        input_values: impl Into<SessionInputs<'i, 'v, N>>,
    ) -> ort::Result<SessionOutputs<'s>> {
        let (ses, idx) = self.get_session()?;
        let ses: &'s mut Session = unsafe { &mut *(&mut *ses.lock() as *mut Session) };
        let out = ses.run(input_values);
        self.release_session(idx);
        out
    }

    pub fn run_with_options<
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
    ) -> Result<SessionOutputs<'r>, ort::Error> {
        let (ses, idx) = self.get_session()?;
        let ses: &'s mut Session = unsafe { &mut *(&mut *ses.lock() as *mut Session) };
        let out = ses.run_with_options(input_values, run_options);
        self.release_session(idx);
        out
    }
}
