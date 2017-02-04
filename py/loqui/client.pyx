import Queue
import socket

import gevent
import time
from gevent.lock import RLock
from gevent.event import AsyncResult

from exceptions import ConnectionError
from socket_session cimport LoquiSocketSession
from exponential_backoff cimport Backoff
import logging

from encoders import ENCODERS

cdef class LoquiClient:
    cdef LoquiSocketSession _session
    cdef object _session_lock
    cdef object _push_handler
    cdef object _connect_greenlet
    cdef object _push_queue
    cdef tuple _address
    cdef object _encoders
    cdef int _connect_timeout
    cdef Backoff _backoff

    def __init__(self, address, push_handler=None, connect_timeout=5, max_push_queue_len=5000, encoders=ENCODERS):
        self._session = None
        self._encoders = encoders
        self._session_lock = RLock()
        self._address = address
        self._connect_timeout = connect_timeout
        self._push_handler = push_handler
        self._connect_greenlet = None
        self._push_queue = Queue.deque(maxlen=max_push_queue_len)
        self._backoff = Backoff(min_delay=0.25, max_delay=2)

    cpdef set_push_handler(self, object push_handler):
        self._push_handler = push_handler
        if self._session:
            self._session.set_push_handler(push_handler)

    cdef inline _clear_current_session(self):
        cdef LoquiSocketSession session = self._session
        with self._session_lock:
            if session is self._session:
                self._backoff.fail()
                self._session = None

    cdef inline _connect_async(self):
        # If we are connected and ready, return true so that we send to the session right away.
        if self._session is not None and not self._session.defunct() and self._session.is_ready():
            return True

        # Otherwise, if the session is established, but not yet ready, return false so that we append
        # to the queue, which will be consumed when we're ready.
        if self._session is not None and not self._session.defunct() and not self._session.is_ready():
            return False

        # Otherwise, we need to connect. Spawn a greenlet to do the connection.
        if self._connect_greenlet is None:
            self._connect_greenlet = gevent.spawn(self._connect_in_greenlet)

        # Return false to signify we should append to the queue.
        return False

    cpdef _connect_and_send_request_async(self, request_data):
        self.connect()
        response = self._session.send_request(request_data)
        return response.get()

    cdef inline _is_socket_connected(self):
        return self._session is not None and not self._session.defunct()

    def _connect_in_greenlet(self):
        try:
            self.connect()
        finally:
            self._connect_greenlet = None

    cdef connect(self):
        cdef LoquiSocketSession session

        if self._session is not None:
            if self._session.defunct():
                self._clear_current_session()
            else:
                return

        with self._session_lock:
            if self._session is not None:
                return

            if self._backoff.fails():
                gevent.sleep(self._backoff.current())

            start = time.time()

            try:
                logging.info('[loqui %s:%s] connecting', self._address[0], self._address[1])
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                sock.settimeout(self._connect_timeout)
                sock.connect(self._address)
                self.handle_new_socket(sock)
                sock.setblocking(False)
                session = LoquiSocketSession(sock, encoders=self._encoders, on_push=self._push_handler)
                elapsed = (time.time() - start) * 1000
                logging.info('[loqui %s:%s] connected in %.2f ms.', self._address[0], self._address[1], elapsed)
                session.await_ready()

                if not session.defunct() and self._push_queue:
                    logging.info('[loqui %s:%s] flushing %i push events.', self._address[0], self._address[1],
                                 len(self._push_queue))

                    while self._push_queue:
                        session.send_push(self._push_queue.popleft())

                self._backoff.succeed()
                self._session = session
            except socket.error:
                logging.exception('[loqui %s:%s] connection failed after %.2f ms.',
                                  self._address[0], self._address[1], (time.time() - start) * 1000)
                self._backoff.fail()
                raise ConnectionError('No connection available')

    cpdef send_request_async(self, request_data):
        if not self._is_socket_connected():
            async_result = AsyncResult()
            gevent.spawn(self._connect_and_send_request_async, request_data, async_result).rawlink(async_result)
            return async_result

        return self._session.send_request(request_data)

    cpdef send_request(self, request_data, timeout=None, retries=3):
        try:
            self.connect()
            response = self._session.send_request(request_data)
            return response.get(timeout=timeout)
        except ConnectionError:
            if retries:
                return self.send_request(request_data, timeout, retries - 1)

            raise

    cpdef send_push(self, push_data):
        if not self._connect_async():
            self._push_queue.append(push_data)

        else:
            self._session.send_push(push_data)

    def handle_new_socket(self, socket):
        pass


cdef class LoquiHTTPUpgradeClient(LoquiClient):
    def handle_new_socket(self, sock):
        upgrade_payload = '\r\n'.join([
            b'GET /_rpc HTTP/1.1',
            (b'Host: %s' % self._address[0]),
            b'Upgrade: loqui',
            b'Connection: Upgrade',
            b'',
            b''
        ])

        sock.sendall(upgrade_payload)
        expected_handshake_responses = [
            '\r\n'.join([
                b'HTTP/1.1 101 Switching Protocols',
                b'Upgrade: loqui',
                b'Connection: Upgrade',
                b'',
                b''
            ]).lower(),
            '\r\n'.join([
                b'HTTP/1.1 101 Switching Protocols',
                b'Connection: Upgrade',
                b'Upgrade: loqui',
                b'',
                b''
            ]).lower(),
        ]

        bytes_remaining = len(expected_handshake_responses[0])
        handshake_response = b''
        while bytes_remaining:
            buf = sock.recv(bytes_remaining)
            if not buf:
                raise ConnectionError('Connection died while reading handshake response')

            handshake_response += buf
            bytes_remaining -= len(buf)

        if handshake_response.lower() not in expected_handshake_responses:
            raise ConnectionError('Invalid handshake response: %s' % handshake_response)
