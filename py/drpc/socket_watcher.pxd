cdef class SocketWatcher:
    cdef bint sock_read_ready
    cdef bint sock_write_ready
    cdef bint sock_write_blocked
    cdef bint is_switching
    cdef int sock_fileno
    cdef object hub
    cdef object waiter
    cdef object waiter_switch
    cdef object waiter_get
    cdef object waiter_clear

    cpdef void mark_ready(self, int fileno, bint read=?)
    cpdef request_switch(self)
    cpdef _request_switch(self)

    cdef reset(self)
    cdef wait(self)
    cdef switch_if_write_unblocked(self)
