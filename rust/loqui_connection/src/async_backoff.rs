use crate::LoquiError;
use backoff::{backoff::Backoff, exponential::ExponentialBackoff};
use failure::Error;
use futures_timer::Delay;
use std::time::Duration;
use tokio::await;

const BACKOFF_INITIAL_INTERVAL: Duration = Duration::from_secs(2);
const BACKOFF_MULTIPLIER: f64 = 2.0;
const BACKOFF_MAX_INTERVAL: Duration = Duration::from_secs(15);

/// A futures safe backoff.
pub struct AsyncBackoff {
    inner: ExponentialBackoff<backoff::SystemClock>,
}

impl AsyncBackoff {
    pub fn new() -> Self {
        Self {
            inner: ExponentialBackoff {
                initial_interval: BACKOFF_INITIAL_INTERVAL,
                multiplier: BACKOFF_MULTIPLIER,
                max_interval: BACKOFF_MAX_INTERVAL,
                // Setting this to Some(duration) will result in the backoff eventually
                // no longer doing anything. i.e. inner.next_backoff() will return None!
                max_elapsed_time: None,
                ..Default::default()
            },
        }
    }

    /// Snooze the current future.
    pub async fn snooze(&mut self) -> Result<(), Error> {
        let backoff_duration = self
            .inner
            .next_backoff()
            .ok_or_else(|| LoquiError::ReachedMaxBackoffElapsedTime)?;
        debug!("Backing off. duration={:?}", backoff_duration);
        await!(Delay::new(backoff_duration))?;
        Ok(())
    }

    /// Reset the backoff.
    pub fn reset(&mut self) {
        self.inner.reset()
    }
}
