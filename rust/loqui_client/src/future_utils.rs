use std::future::Future;
use tokio_async_await::compat::backward::Compat;
use tokio_current_thread::{block_on_all as block_on_all_old, spawn as spawn_old};

pub fn block_on_all<O, E>(future: impl Future<Output = Result<O, E>>) -> Result<O, E> {
    block_on_all_old(Compat::new(future))
}

pub fn spawn<F>(future: F)
where
    F: Future<Output = Result<(), ()>> + 'static,
{
    spawn_old(Compat::new(future))
}
