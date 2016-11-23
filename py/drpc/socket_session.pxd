from libc.stdint cimport uint32_t, uint8_t

from stream_handler cimport DRPCStreamHandler
from socket_watcher cimport SocketWatcher
from opcodes cimport Ping, Pong, Hello, Request, Response, Push, SelectEncoding, GoAway

cdef class DRPCSocketSession:
    cdef DRPCStreamHandler _stream_handler
    cdef object _sock
    cdef SocketWatcher _watcher
    cdef dict _inflight_requests
    cdef bint _is_client

    cdef bint _shutting_down
    cdef object _shutdown_event

    cdef object _close_event

    cdef bint _is_ready
    cdef object _ready_event

    cdef uint32_t _ping_interval
    cdef object _available_encoders

    cdef object _on_request
    cdef object _on_push
    cdef object _encoder_loads
    cdef object _encoder_dumps
    cdef bytes _encoding

    cpdef set_push_handler(self, object push_handler)
    cpdef join(self, timeout=?)
    cpdef terminate(self)
    cpdef close(self, uint8_t code=?, bytes reason=?, block=?, block_timeout=?, close_timeout=?,
                bint via_remote_goaway=?)

    cdef bint defunct(self)

    cdef _resume_sending(self)
    cpdef _close_timeout(self)
    cdef shutdown(self)
    cdef void _cleanup_socket(self)
    cdef _cleanup_inflight_requests(self, close_exception)
    cdef _encode_data(self, object data)
    cdef _decode_data(self, object data)
    cpdef object send_request(self, object data)
    cpdef object send_push(self, object data)
    cpdef object send_response(self, uint32_t seq, object data)
    cpdef object send_ping(self)
    cpdef object _send_select_encoding(self, bytes encoding)
    cpdef object _send_hello(self)
    cdef _handle_ping_timeout(self)
    cdef _handle_data_received(self, data)
    cdef _handle_client_events(self, list events)
    cdef _handle_server_events(self, list events)
    cdef _handle_request(self, Request request)
    cdef _handle_response(self, Response response)
    cdef _handle_push(self, Push push)
    cdef _handle_ping(self, Ping ping)
    cdef _handle_pong(self, Pong pong)
    cdef _handle_hello(self, Hello hello)
    cdef _handle_select_encoding(self, SelectEncoding select_encoding)
    cdef _handle_go_away(self, GoAway go_away)
    cdef _pick_best_encoding(self, list encodings)
    cpdef _ping_loop(self)
    cpdef _run_loop(self)
