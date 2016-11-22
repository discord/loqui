import socket
from gevent.lock import RLock
from gevent.event import Event

from drpc.exceptions import ConnectionError
from socket_session cimport DRPCSocketSession
from exponential_backoff cimport Backoff

from encoders import ENCODERS

cdef class DRPCClient:
    cdef DRPCSocketSession _session
    cdef object _session_lock
    cdef object _session_event
    cdef tuple _address
    cdef int _connect_timeout
    cdef Backoff _backoff

    def __init__(self, address):
        self._session = None
        self._session_lock = RLock()
        self._session_event = Event()
        self._address = address
        self._connect_timeout = 5
        self._backoff = Backoff(min_delay=0.25, max_delay=2)

    cdef inline _clear_current_session(self):
        cdef DRPCSocketSession session = self._session
        session = self._session
        with self._session_lock:
            if session is self._session:
                self._backoff.fail()
                self._session_event.clear()
                self._session = None

    cdef connect(self):
        if self._session is not None:
            if self._session.defunct():
                self._clear_current_session()
            else:
                return

        if self._backoff.fails():
            self._session_event.wait(self._backoff.current())
            if self._session is not None:
                return

        with self._session_lock:
            if self._session is not None:
                return

            try:
                print ('connect, address:%s, port:%s', self._address[0], self._address[1])
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                sock.settimeout(self._connect_timeout)
                sock.connect(self._address)
                self.handle_new_socket(sock)
                sock.setblocking(False)
                self._session = DRPCSocketSession(sock, ENCODERS)
                self._backoff.succeed()
                self._session_event.set()
                print 'connected'

            except socket.error:
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


cdef class DRPCHTTPUpgradeCient(DRPCClient):
    def handle_new_socket(self, sock):
        upgrade_payload = '\r\n'.join([
            b'GET /_rpc HTTP/1.1',
            (b'Host: %s' % self._address[0]),
            b'Upgrade: drpc',
            b'Connection: Upgrade',
            b'',
            b''
        ])

        sock.sendall(upgrade_payload)
        expected_handshake_response = '\r\n'.join([
            b'HTTP/1.1 101 Switching Protocols',
            b'Upgrade: drpc',
            b'Connection: Upgrade',
            b'',
            b''
        ])

        bytes_remaining = len(expected_handshake_response)
        handshake_response = b''
        while bytes_remaining:
            buf = sock.recv(bytes_remaining)
            if not buf:
                raise ConnectionError('Connection died while reading handshake response')

            handshake_response += buf
            bytes_remaining -= len(buf)

        if handshake_response != expected_handshake_response:
            raise ConnectionError('Invalid handshake response: %s' % handshake_response)
