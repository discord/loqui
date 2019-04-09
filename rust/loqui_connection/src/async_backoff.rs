use backoff::{backoff::Backoff, exponential::ExponentialBackoff};
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
                max_elapsed_time: None,
                ..Default::default()
            },
        }
    }

    /// Snooze the current future.
    pub async fn snooze(&mut self) {
        // TODO: expect
        let backoff_duration = self.inner.next_backoff().expect("missing backoff");
        debug!("Backing off. duration={:?}", backoff_duration);
        let result = await!(Delay::new(backoff_duration));
        debug!("delay result {:?}", result)
    }

    /// Reset the backoff.
    pub fn reset(&mut self) {
        self.inner.reset()
    }
}
