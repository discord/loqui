use std::future::Future;
use std::pin::Pin;

/// Trait implemented by servers for handling individual `Request`s and `Push`es.
pub trait RequestHandler: Send + Sync + 'static {
    /// Handle a single request. Return a future with the result.
    fn handle_request(
        &self,
        payload: Vec<u8>,
        encoding: &'static str,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send>>;
    /// Handle a single push.
    fn handle_push(
        &self,
        payload: Vec<u8>,
        encoding: &'static str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}
