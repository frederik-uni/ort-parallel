#![doc = include_str!("../README.md")]

#[cfg(feature = "async")]
mod async_pool;
#[cfg(feature = "sync")]
mod semaphore;
#[cfg(feature = "sync")]
mod sync_pool;

#[cfg(feature = "async")]
pub use async_pool::AsyncSessionPool;

use ort::session::builder::SessionBuilder;
#[cfg(feature = "sync")]
pub use sync_pool::SessionPool;

struct SessionBuilderFactory(SessionBuilder);

impl SessionBuilderFactory {
    pub(crate) fn generate(&self) -> SessionBuilder {
        self.0.clone()
    }
}

unsafe impl Sync for SessionBuilderFactory {}
unsafe impl Send for SessionBuilderFactory {}
