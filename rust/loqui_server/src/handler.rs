use loqui_protocol::Request;
use failure::Error;
use std::future::Future;
use std::pin::Pin;

pub trait Handler: Send + Sync {
    fn handle_request(
        &self,
        request: Request,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send>>;
}
