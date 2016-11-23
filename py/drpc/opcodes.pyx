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
    def __cinit__(self, uint8_t version, uint32_t ping_interval, list supported_encodings):
        self.version = version
        self.ping_interval = ping_interval
        self.supported_encodings = supported_encodings

cdef class GoAway:
    def __cinit__(self, uint8_t code, bytes reason):
        self.code = code
        self.reason = reason

cdef class SelectEncoding:
    def __cinit__(self, bytes encoding):
        self.encoding = encoding

cdef class Error:
    def __cinit__(self, uint8_t code, uint32_t seq, bytes data):
        self.code = code
        self.seq = seq
        self.data = data
