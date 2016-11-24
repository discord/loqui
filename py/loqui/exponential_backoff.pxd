
cdef class Backoff:
    cdef float _min
    cdef float _max
    cdef float _current
    cdef int _fails
    cdef bint _jitter

    cdef int fails(self)
    cdef float current(self)
    cdef void succeed(self)
    cdef float fail(self)
