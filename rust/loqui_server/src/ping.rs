use super::connection::Event;
use failure::Error;
use futures_timer::Interval;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::prelude::*;

const DEFAULT_INTERVAL_MS: usize = 5000;

pub struct Ping {
    handle: Handle,
    inner_stream: Option<Interval>,
}

#[derive(Clone)]
pub struct Handle {
    pub ready: Arc<AtomicBool>,
    pub interval_ms: Arc<AtomicUsize>,
}

impl Handle {
    pub fn start(&self, interval_ms: u32) {
        self.ready.store(true, Ordering::Relaxed);
        self.interval_ms
            .store(interval_ms as usize, Ordering::SeqCst);
    }

    fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
}

impl Ping {
    pub fn new() -> Self {
        Self {
            handle: Handle {
                ready: Arc::new(AtomicBool::new(false)),
                interval_ms: Arc::new(AtomicUsize::new(DEFAULT_INTERVAL_MS)),
            },
            inner_stream: None,
        }
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }
}

impl Stream for Ping {
    type Item = Event;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Error> {
        match &mut self.inner_stream {
            Some(stream) => match stream.poll() {
                Ok(Async::Ready(Some(()))) => Ok(Async::Ready(Some(Event::Ping))),
                Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(e) => Err(Error::from(e)),
            },
            None => {
                if self.handle.is_ready() {
                    let interval_ms = self.handle.interval_ms.load(Ordering::SeqCst);
                    let duration = Duration::from_millis(interval_ms as u64);
                    let stream = Interval::new(duration);
                    self.inner_stream = Some(stream);
                    Ok(Async::Ready(Some(Event::Ping)))
                } else {
                    Ok(Async::NotReady)
                }
            }
        }
    }
}
