use super::connection::{Connection, Event, EventHandler, HandleEventResult};
use super::frame_handler::{FrameHandler, ServerFrameHandler};
use futures::sync::mpsc::{self, UnboundedSender};
use loqui_protocol::codec::LoquiFrame;
use std::sync::Arc;
use tokio::await as tokio_await;
use tokio::prelude::*;

#[derive(Debug)]
pub enum ServerEvent {
    Complete(LoquiFrame),
}

pub struct ServerEventHandler {
    frame_handler: Arc<dyn FrameHandler>,
    tx: UnboundedSender<Event<ServerEvent>>,
}

impl ServerEventHandler {
    pub fn new(
        tx: UnboundedSender<Event<ServerEvent>>,
        frame_handler: Arc<dyn FrameHandler>,
    ) -> Self {
        Self { tx, frame_handler }
    }
}

impl EventHandler<ServerEvent> for ServerEventHandler {
    fn handle_event(&mut self, event: Event<ServerEvent>) -> HandleEventResult {
        let frame_handler = self.frame_handler.clone();
        let tx = self.tx.clone();
        Box::new(
            async move {
                match event {
                    Event::Socket(frame) => {
                        tokio::spawn_async(
                            async move {
                                // TODO: handle error
                                match await!(Box::into_pin(frame_handler.handle_frame(frame))) {
                                    Ok(Some(frame)) => {
                                        tokio_await!(
                                            tx.send(Event::Internal(ServerEvent::Complete(frame)))
                                        );
                                    }
                                    Ok(None) => {
                                        dbg!("None");
                                    }
                                    Err(e) => {
                                        dbg!(e);
                                    }
                                }
                            },
                        );
                        Ok(None)
                    }
                    Event::Internal(ServerEvent::Complete(frame)) => Ok(Some(frame)),
                }
            },
        )
    }
}
