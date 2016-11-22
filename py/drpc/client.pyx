import socket
from gevent.lock import RLock
from gevent.event import Event

from drpc.exceptions import ConnectionError
from socket_session cimport DRPCSocketSession

from encoders import ENCODERS

cdef class DRPCClient:
    cdef DRPCSocketSession _session
    cdef object _session_lock
    cdef object _session_event
    cdef tuple _address
    cdef int _connect_timeout

    def __init__(self, address):
        self._session = None
        self._session_lock = RLock()
        self._session_event = Event()
        self._address = address
        self._connect_timeout = 5

    cdef connect(self):
        if self._session is not None:
            return

        # if self._backoff.fails:
        #     self._session_event.wait(self._backoff.current)
        #    if self._session is not None:
        #        return

        with self._session_lock:
            if self._session is not None:
                return

            try:
                print ('connect, address:%s, port:%s', self._address[0], self._address[1])
                sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                sock.settimeout(self._connect_timeout)
                sock.connect(self._address)
                sock.setblocking(False)
                self._session = DRPCSocketSession(sock, ENCODERS)
                # self._backoff.succeed()
                self._session_event.set()
                print 'connected'

            except socket.error:
                # self._backoff.fail()
                raise ConnectionError('No connection available')

    cpdef send_request(self, request_data, timeout=None):
        self.connect()
        response = self._session.send_request(request_data)
        return response.get(timeout=timeout)

    cpdef send_push(self, push_data):
        self.connect()
        self._session.send_push(push_data)
