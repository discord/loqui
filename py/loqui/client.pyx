import socket

import gevent
from gevent.lock import RLock

from exceptions import ConnectionError
from socket_session cimport LoquiSocketSession
from exponential_backoff cimport Backoff
import logging

from encoders import ENCODERS

cdef class LoquiClient:
    cdef LoquiSocketSession _session
    cdef object _session_lock
    cdef object _push_handler
    cdef tuple _address
    cdef int _connect_timeout
    cdef Backoff _backoff

    def __init__(self, address, push_handler=None, connect_timeout=5):
        self._session = None
        self._session_lock = RLock()
        self._address = address
        self._connect_timeout = connect_timeout
        self._push_handler = push_handler
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

    cdef connect(self):
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

            try:
                logging.info('[Loqui] Connecting to %s:%s', self._address[0], self._address[1])
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                sock.settimeout(self._connect_timeout)
                sock.connect(self._address)
                self.handle_new_socket(sock)
                sock.setblocking(False)
                self._session = LoquiSocketSession(sock, ENCODERS, on_push=self._push_handler)
                self._backoff.succeed()
                logging.info('[Loqui] Connected to %s:%s', self._address[0], self._address[1])

            except socket.error:
                logging.exception('[Loqui] Connection to %s:%s failed.', self._address[0], self._address[1])
                self._backoff.fail()
                raise ConnectionError('No connection available')

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
        self.connect()
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
