use loqui_connection::EncoderFactory;
use std::future::Future;

/// Trait implemented by servers for handling individual `Request`s and `Push`es.
pub trait RequestHandler: Send + Sync + 'static {
    type EncoderFactory: EncoderFactory;
    /// Future returned by the handle_request function that will be executed. The `Output`
    /// will be returned back over the socket.
    type RequestFuture: Send + Future<Output = <Self::EncoderFactory as EncoderFactory>::Encoded>;
    /// Future executed asynchronously for a push request.
    type PushFuture: Send + Future<Output = ()>;
    /// Handle a single request. Return a future with the result.
    fn handle_request(
        &self,
        request: <Self::EncoderFactory as EncoderFactory>::Decoded,
    ) -> Self::RequestFuture;
    /// Handle a single push.
    fn handle_push(
        &self,
        request: <Self::EncoderFactory as EncoderFactory>::Decoded,
    ) -> Self::PushFuture;
}
