from libc.stdint cimport uint32_t, uint8_t

cdef class Response:
    def __cinit__(self, uint32_t seq, bytes data):
        self.seq = seq
        self.data = data

cdef class Request:
    def __cinit__(self, uint32_t seq, bytes data):
        self.seq = seq
        self.data = data

cdef class Push:
    def __cinit__(self, bytes data):
        self.data = data

cdef class Ping:
    def __cinit__(self, uint32_t seq):
        self.seq = seq

cdef class Pong:
    def __cinit__(self, uint32_t seq):
        self.seq = seq

cdef class Hello:
    def __cinit__(self, uint32_t seq, uint32_t ping_interval):
        self.seq = seq
        self.ping_interval = ping_interval

cdef class GoAway:
    def __cinit__(self, uint8_t code, bytes data):
        self.code = code
        self.data = data