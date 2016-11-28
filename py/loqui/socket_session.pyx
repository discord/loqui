import errno
import logging
import socket

import gevent
from gevent.event import AsyncResult, Event

from libc.stdint cimport uint32_t, uint8_t

from loqui.exceptions import NoEncoderAvailable, ConnectionTerminated, LoquiDecoderError, StreamDefunct, \
    NotClientException, NotServerException, LoquiErrorReceived
from opcodes cimport Request, Response, Ping, Pong, Push, Hello, GoAway, HelloAck, Error

from socket_watcher cimport SocketWatcher
from stream_handler cimport LoquiStreamHandler

cdef size_t OUTBUF_MAX = 65535


class CloseReasons(object):
    NORMAL = 0
    PING_TIMEOUT = 1
    UNKNOWN_ENCODER = 2
    NO_MUTUAL_ENCODERS = 3
    DIDNT_STOP_IN_TIME = 4
    DECODER_ERROR = 5

cdef class LoquiSocketSession:
    def __cinit__(self, object sock, object encoders, bint is_client=True, object on_request=None, object on_push=None):
        self._is_client = is_client
        self._stream_handler = LoquiStreamHandler()
        self._sock = sock
        self._watcher = SocketWatcher(self._sock.fileno())
        self._inflight_requests = {}
        self._available_encoders = encoders
        self._available_compressors = {}
        self._shutdown_event = Event()
        self._close_event = Event()
        self._ready_event = Event()
        self._ping_interval = 5
        self._is_ready = False
        self._shutting_down = False

        if not self._is_client:
            self._on_request = on_request

        self._on_push = on_push

        gevent.spawn(self._ping_loop)
        gevent.spawn(self._run_loop)

        if is_client:
            self._send_hello()

    cpdef set_push_handler(self, object push_handler):
        self._on_push = None

    cdef _resume_sending(self):
        if self._sock is None:
            return

        if self._stream_handler.write_buffer_len() == 0:
            return

        self._watcher.switch_if_write_unblocked()

    cdef shutdown(self):
        if self._shutting_down:
            return

        self._shutting_down = True

    cdef void _cleanup_socket(self):
        sock = self._sock
        self._sock = None
        if sock:
            sock.close()

        # Request a switch so that the socket event loop will exit.
        if sock:
            self._watcher.request_switch()

        self._cleanup_inflight_requests(ConnectionTerminated())

        # Unblock anything waiting on ready event.
        if not self._ready_event.is_set():
            self._ready_event.set()

        # Unblock anything waiting on stop event.
        if not self._shutdown_event.is_set():
            self._shutdown_event.set()

        # Unblock anything waiting on the close event.
        if not self._close_event.is_set():
            self._close_event.set()

    cpdef close(self, uint8_t code=CloseReasons.NORMAL, bytes reason=None,
                block=False, block_timeout=None, close_timeout=None, bint via_remote_goaway=False):

        if not self._shutting_down:
            self._shutting_down = True

            # Unblock anything waiting on ready event.
            if not self._ready_event.is_set():
                self._ready_event.set()

            # Unblock anything waiting on stop event.
            if not self._shutdown_event.is_set():
                self._shutdown_event.set()

            # Send goaway to the remote node.
            if not via_remote_goaway and self._sock:
                self._stream_handler.send_goaway(0, code, reason)

            # Spawn a greenlet that will wait
            gevent.spawn(self._close_timeout)

        # If we are blocking, wait on close event to succeed.
        if block:
            self.join(block_timeout)

    cpdef _close_timeout(self):
        if self._close_event.wait(self._ping_interval):
            return

        self._cleanup_socket()

    cpdef join(self, timeout=None):
        self._close_event.wait(timeout=timeout)

    cpdef terminate(self):
        self._cleanup_socket()

    cdef _cleanup_inflight_requests(self, close_exception):
        requests = self._inflight_requests.values()
        self._inflight_requests.clear()

        for request in requests:
            if isinstance(request, AsyncResult):
                request.set_exception(close_exception)

    cdef _encode_data(self, object data):
        if not self._is_ready:
            self._ready_event.wait()

        if not self._encoder_dumps:
            raise NoEncoderAvailable()

        return 0, self._encoder_dumps(data)

    cdef _decode_data(self, uint8_t flags, object data):
        if not self._is_ready:
            self._ready_event.wait()

        if not self._encoder_loads:
            raise NoEncoderAvailable()

        return self._encoder_loads(data)

    cpdef object send_request(self, object data):
        if not self._is_client:
            raise NotClientException()

        if self.defunct():
            raise StreamDefunct

        cdef bytes encoded_data
        cdef uint8_t flags
        flags, encoded_data = self._encode_data(data)

        result = AsyncResult()
        cdef uint32_t seq = self._stream_handler.send_request(flags, encoded_data)
        self._inflight_requests[seq] = result
        self._resume_sending()
        return result

    cpdef object send_push(self, object data):
        if self.defunct():
            raise StreamDefunct()

        cdef bytes encoded_data
        cdef uint8_t flags
        flags, encoded_data = self._encode_data(data)

        self._stream_handler.send_push(flags, encoded_data)
        self._resume_sending()

    cpdef object send_response(self, uint32_t seq, object data):
        if self._is_client:
            raise NotServerException()

        if self.defunct():
            return

        cdef bytes encoded_data
        cdef uint8_t flags

        request = self._inflight_requests.pop(seq)
        if request:
            flags, encoded_data = self._encode_data(data)
            self._stream_handler.send_response(flags, seq, encoded_data)
            self._resume_sending()

        else:
            logging.error('Sending response for unknown seq %s', seq)

        return None

    cpdef object send_ping(self):
        if self.defunct():
            raise StreamDefunct()

        result = AsyncResult()
        cdef uint32_t seq = self._stream_handler.send_ping(0)
        self._inflight_requests[seq] = result
        self._resume_sending()
        return result

    cpdef object _send_hello_ack(self, bytes selected_encoding, bytes selected_compressor):
        if self._is_client:
            raise NotServerException()

        if self.defunct():
            raise StreamDefunct()

        self._stream_handler.send_hello_ack(0, int(self._ping_interval * 1000), selected_encoding, selected_compressor)

    cpdef object _send_hello(self):
        if not self._is_client:
            raise NotClientException()

        self._stream_handler.send_hello(0, list(self._available_encoders.keys()), list(self._available_compressors.keys()))

    cdef _handle_ping_timeout(self):
        self.close(reason=CloseReasons.PING_TIMEOUT)

    cdef _handle_data_received(self, data):
        try:
            events = self._stream_handler.on_bytes_received(data)

        except LoquiDecoderError as e:
            print e
            return self.close(code=CloseReasons.DECODER_ERROR)

        if not events:
            return

        if self._is_client:
            self._handle_client_events(events)

        else:
            self._handle_server_events(events)

        self._resume_sending()

    cdef _handle_client_events(self, list events):
        for event in events:
            if isinstance(event, Response):
                self._handle_response(event)

            elif isinstance(event, Push):
                self._handle_push(event)

            elif isinstance(event, Ping):
                self._handle_ping(event)

            elif isinstance(event, Pong):
                self._handle_pong(event)

            elif isinstance(event, GoAway):
                self._handle_go_away(event)

            elif isinstance(event, HelloAck):
                self._handle_hello_ack(event)

            elif isinstance(event, Error):
                self._handle_error(event)

    cdef _handle_server_events(self, list events):
        for event in events:
            if isinstance(event, Request):
                self._handle_request(event)

            elif isinstance(event, Push):
                self._handle_push(event)

            elif isinstance(event, Ping):
                self._handle_ping(event)

            elif isinstance(event, Pong):
                self._handle_pong(event)

            elif isinstance(event, GoAway):
                self._handle_go_away(event)

            elif isinstance(event, Hello):
                self._handle_hello(event)

    cdef _handle_request(self, Request request):
        if self._on_request:
            # In this case, we set the inflight requests to the given request. That way send_response
            # will know if the seq is valid or not.
            request.data = self._decode_data(request.flags, request.data)
            self._inflight_requests[request.seq] = request
            response = self._on_request(request, self)
            # If a response is given, we can return it to the sender right away.
            # Otherwise, it's the responsibility of the `_on_request` handler to eventually
            # call `send_response`.
            if response is not None:
                self.send_response(request.seq, response)

    cdef _handle_response(self, Response response):
        request = self._inflight_requests.pop(response.seq)
        if request:
            try:
                # If we've gotten a response for a request we've made.
                request.set(self._decode_data(response.flags, response.data))
            except Exception as e:
                request.set_exception(e)

    cdef _handle_push(self, Push push):
        if self._on_push:
            push.data = self._decode_data(push.flags, push.data)
            self._on_push(push, self)

    cdef _handle_ping(self, Ping ping):
        # Nothing to do here - the stream handler handles sending pongs back for us.
        pass

    cdef _handle_pong(self, Pong pong):
        ping_request = self._inflight_requests.pop(pong.seq)
        if ping_request:
            ping_request.set(pong)

    cdef _handle_hello(self, Hello hello):
        encoding, encoder = self._pick_best_encoding(hello.supported_encodings)

        if not encoding:
            self.close(reason=CloseReasons.NO_MUTUAL_ENCODERS)

        else:
            self._encoder_dumps = encoder.dumps
            self._encoder_loads = encoder.loads
            self._encoding = encoding
            self._send_hello_ack(encoding, None)
            self._is_ready = True
            self._ready_event.set()

    cdef _handle_hello_ack(self, HelloAck hello_ack):
        encoder = self._available_encoders.get(hello_ack.selected_encoding)
        if not encoder:
            self.close(code=CloseReasons.UNKNOWN_ENCODER, reason=b'Unknown encoding %s' % hello_ack.selected_encoding)

        else:
            self._ping_interval = int(hello_ack.ping_interval / 1000)
            self._encoder_dumps = encoder.dumps
            self._encoder_loads = encoder.loads
            self._encoding = hello_ack.selected_encoding
            self._is_ready = True
            self._ready_event.set()

    cdef _handle_go_away(self, GoAway go_away):
        print "Got goaway on seq", self._stream_handler.current_seq(), go_away.code
        self.close(via_remote_goaway=True, reason=go_away.reason, code=go_away.code)

    cdef _handle_error(self, Error error):
        request = self._inflight_requests.pop(error.seq)
        if request:
            request.set_exception(LoquiErrorReceived(error.code, error.data))

    cdef _pick_best_encoding(self, list encodings):
        for encoding in encodings:
            encoder = self._available_encoders.get(encoding)
            if encoder:
                return encoding, encoder

        return None, None

    cpdef _ping_loop(self):
        while True:
            ping_result = self.send_ping()
            if self._shutdown_event.wait(self._ping_interval):
                return

            if not ping_result.ready():
                self._handle_ping_timeout()

    cpdef _run_loop(self):
        loop = gevent.get_hub().loop
        io = loop.io
        cdef int MAXPRI = loop.MAXPRI
        cdef int READ = 1
        cdef int WRITE = 2
        cdef bint write_watcher_started = False
        cdef bint sock_should_write = False
        cdef bint did_empty_buffer = False
        cdef object sock_recv = self._sock.recv
        cdef object sock_send = self._sock.send
        cdef object watcher_mark_ready = (<object> self._watcher).mark_ready
        cdef size_t write_bytes_remaining

        sock_read_watcher = io(self._watcher.sock_fileno, READ)
        sock_write_watcher = io(self._watcher.sock_fileno, WRITE)

        try:
            sock_read_watcher.start(watcher_mark_ready, self._watcher.sock_fileno, True)

            while self._sock:
                if write_watcher_started == False and self._stream_handler.write_buffer_len() > 0:
                    sock_write_watcher.start(watcher_mark_ready, self._watcher.sock_fileno, False)

                self._watcher.wait()
                if not self._sock:
                    return

                if self._watcher.sock_read_ready:
                    try:
                        data = sock_recv(65536)
                    except socket.error as e:
                        if e.errno in (errno.EAGAIN, errno.EINPROGRESS):
                            continue

                        data = None

                    if not data:
                        self.close()
                        self._cleanup_socket()
                        return

                    else:
                        self._handle_data_received(data)

                # We should attempt to write, if the watcher has notified us that the socket is ready
                # to accept more data, or we aren't write blocked yet - and we have a write buffer.
                sock_should_write = self._watcher.sock_write_ready or (
                    not self._watcher.sock_write_blocked and self._stream_handler.write_buffer_len() > 0
                )

                if sock_should_write:
                    try:
                        bytes_written = sock_send(self._stream_handler.write_buffer_get_bytes(OUTBUF_MAX, False))
                    except socket.error as e:
                        if e.errno in (errno.EAGAIN, errno.EINPROGRESS):
                            bytes_written = 0

                        else:
                            self.close()
                            self._cleanup_socket()

                    # No bytes have been written. It's safe to assume the socket is still (somehow) blocked
                    # and we don't need to do anything.
                    if not bytes_written:
                        self._watcher.sock_write_blocked = True
                        continue

                    write_bytes_remaining = self._stream_handler.write_buffer_consume_bytes(bytes_written)
                    # Did we completely write the buffer? If so - the socket isn't blocked anymore.
                    self._watcher.sock_write_blocked = write_bytes_remaining > 0

                    # If no data is available to send - we can stop the watcher. Otherwise,
                    # we will leave the watcher open, so the data filled into the buffer
                    # will attempt to be written upon the next tick of the event loop.
                    if write_bytes_remaining == 0:
                        sock_write_watcher.stop()
                        write_watcher_started = False

                # If we are shutting down, have an empty write buffer, and no more in-flight requests,
                # it's safe to terminate the socket.
                if self._shutting_down and self._stream_handler.write_buffer_len() == 0 and not self._inflight_requests:
                    print 'cleaning up socket because we are drained'
                    self._cleanup_socket()

                self._watcher.reset()

        finally:
            sock_read_watcher.stop()
            sock_write_watcher.stop()

    cdef bint defunct(self):
        if self._sock is None:
            return True

        if self._shutting_down:
            return True

        return False