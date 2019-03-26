use super::connection::Event;
use failure::Error;
use futures_timer::Interval;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::prelude::*;

pub struct Ping {
    handle: Handle,
    stream: Option<Interval>,
}

#[derive(Clone)]
pub struct Handle {
    pub ready: Arc<AtomicBool>,
}

impl Handle {
    pub fn start(&self, interval: u32) {
        self.ready.store(true, Ordering::Relaxed);
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
            },
            stream: None,
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
        match &mut self.stream {
            Some(stream) => match stream.poll() {
                Ok(Async::Ready(Some(()))) => Ok(Async::Ready(Some(Event::Ping))),
                Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(e) => Err(Error::from(e)),
            },
            None => {
                if self.handle.is_ready() {
                    let duration = Duration::from_millis(5000);
                    let stream = Interval::new(duration);
                    self.stream = Some(stream);
                }
                Ok(Async::NotReady)
            }
        }
    }
}
