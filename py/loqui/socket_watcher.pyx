import gevent
from gevent.hub import Waiter
from greenlet import getcurrent

cdef class SocketWatcher:
    def __cinit__(self, int sock_fileno):
        self.sock_fileno = sock_fileno
        self.sock_write_blocked = False
        self.waiter = Waiter()
        self.waiter_get = self.waiter.get
        self.waiter_clear = self.waiter.clear
        self.waiter_switch = lambda: self.waiter.switch(None)
        self.hub = gevent.get_hub()
        self.is_switching = False
        self.reset()

    cpdef void mark_ready(self, int fileno, bint read=True):
        if fileno != self.sock_fileno:
            return

        if read:
            self.sock_read_ready = True

        else:
            self.sock_write_ready = True

        self.waiter_switch()

    cdef reset(self):
        self.sock_read_ready = False
        self.sock_write_ready = False
        self.is_switching = False
        self.waiter_clear()

    cdef wait(self):
        return self.waiter_get()

    cpdef request_switch(self):
        if self.is_switching:
            return

        if self.hub is getcurrent():
            self._request_switch()
        else:
            self.hub.loop.run_callback(self._request_switch)

    cpdef _request_switch(self):
        if self.is_switching:
            return

        self.is_switching = True
        self.waiter_switch()

    cdef switch_if_write_unblocked(self):
        if not self.sock_write_blocked:
            self.request_switch()
