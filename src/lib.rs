#![doc = include_str!("../README.md")]

#[cfg(feature = "async")]
mod async_pool;
#[cfg(feature = "sync")]
mod semaphore;
#[cfg(feature = "sync")]
mod sync_pool;

#[cfg(feature = "async")]
pub use async_pool::AsyncSessionPool;

#[cfg(feature = "sync")]
pub use sync_pool::SessionPool;
