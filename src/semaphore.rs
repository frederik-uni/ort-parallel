use std::sync::{Arc, Condvar, Mutex};

pub struct Semaphore {
    inner: Arc<Inner>,
}

struct Inner {
    count: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    /// Create a new semaphore with `capacity` permits.
    pub fn new(capacity: usize) -> Self {
        Semaphore {
            inner: Arc::new(Inner {
                count: Mutex::new(capacity),
                cvar: Condvar::new(),
            }),
        }
    }

    /// Acquire a permit. Blocks until one is available.
    pub fn acquire(&self) {
        let mut count = self.inner.count.lock().unwrap();
        while *count == 0 {
            count = self.inner.cvar.wait(count).unwrap();
        }
        *count -= 1;
    }

    /// Release a permit.
    pub fn release(&self) {
        let mut count = self.inner.count.lock().unwrap();
        *count += 1;
        self.inner.cvar.notify_one();
    }
}
