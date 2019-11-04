use async_trait::async_trait;

/// Trait implemented by servers for handling individual `Request`s and `Push`es.
#[async_trait]
pub trait RequestHandler: Send + Sync + 'static {
    /// Handle a single request. Return a future with the result.
    async fn handle_request(&self, payload: Vec<u8>, encoding: &'static str) -> Vec<u8>;
    /// Handle a single push.
    async fn handle_push(&self, payload: Vec<u8>, encoding: &'static str);
}
