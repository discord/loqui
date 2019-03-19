use failure::Error;
use loqui_protocol::codec::LoquiFrame;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug)]
pub struct RequestContext {
    pub payload: Vec<u8>,
    pub flags: u8,
    pub encoding: String,
}

pub trait Handler: Send + Sync {
    fn handle_request(
        &self,
        request: RequestContext,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send>>;
}
