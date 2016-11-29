from libc.stdint cimport uint32_t, uint8_t

cdef class Response:
    def __cinit__(self, uint8_t flags, uint32_t seq, bytes data):
        self.flags = flags
        self.seq = seq
        self.data = data

cdef class Request:
    def __cinit__(self, uint8_t flags, uint32_t seq, bytes data):
        self.flags = flags
        self.seq = seq
        self.data = data

cdef class Push:
    def __cinit__(self, uint8_t flags, bytes data):
        self.flags = flags
        self.data = data

cdef class Ping:
    def __cinit__(self, uint8_t flags, uint32_t seq):
        self.flags = flags
        self.seq = seq

cdef class Pong:
    def __cinit__(self, uint8_t flags, uint32_t seq):
        self.flags = flags
        self.seq = seq

cdef class Hello:
    def __cinit__(self, uint8_t flags, uint8_t version, list supported_encodings, list supported_compressions):
        self.flags = flags
        self.version = version
        self.supported_encodings = supported_encodings
        self.supported_compressions = supported_compressions

cdef class HelloAck:
    def __cinit__(self, uint8_t flags, uint32_t ping_interval, bytes selected_encoding, bytes selected_compression):
        self.flags = flags
        self.ping_interval = ping_interval
        self.selected_encoding = selected_encoding
        self.selected_compression = selected_compression

cdef class GoAway:
    def __cinit__(self, uint8_t flags, uint16_t code, bytes reason):
        self.flags = flags
        self.code = code
        self.reason = reason

cdef class Error:
    def __cinit__(self, uint8_t flags, uint16_t code, uint32_t seq, bytes data):
        self.flags = flags
        self.code = code
        self.seq = seq
        self.data = data
