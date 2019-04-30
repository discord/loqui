/// Provides utilities for running futures in tests.
use std::future::Future;
use tokio_async_await::compat::backward::Compat;
use tokio_current_thread::{block_on_all as block_on_all_old, spawn as spawn_old};

/// Run the executor bootstrapping the execution with the provided future.
///
/// This creates a new [`CurrentThread`] executor, spawns the provided future,
/// and blocks the current thread until the provided future and **all**
/// subsequently spawned futures complete. In other words:
///
/// * If the provided bootstrap future does **not** spawn any additional tasks,
///   `block_on_all` returns once `future` completes.
/// * If the provided bootstrap future **does** spawn additional tasks, then
///   `block_on_all` returns once **all** spawned futures complete.
///
/// See [module level][mod] documentation for more details.
///
/// [`CurrentThread`]: struct.CurrentThread.html
/// [mod]: index.html
pub fn block_on_all<F, O, E>(future: F) -> Result<O, E>
where
    F: Future<Output = Result<O, E>>,
{
    block_on_all_old(Compat::new(future))
}

/// Executes a future on the current thread.
///
/// The provided future must complete or be canceled before `run` will return.
///
/// Unlike [`tokio::spawn`], this function will always spawn on a
/// `CurrentThread` executor and is able to spawn futures that are not `Send`.
///
/// # Panics
///
/// This function can only be invoked from the context of a `run` call; any
/// other use will result in a panic.
///
/// [`tokio::spawn`]: ../fn.spawn.html
pub fn spawn<F>(future: F)
where
    F: Future<Output = Result<(), ()>> + 'static,
{
    spawn_old(Compat::new(future))
}
