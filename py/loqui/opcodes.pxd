from libc.stdint cimport uint32_t, uint8_t, uint16_t

cdef class Response:
    cdef readonly uint8_t flags
    cdef readonly uint32_t seq
    cdef readonly bytes data

cdef class Request:
    cdef readonly uint8_t flags
    cdef readonly uint32_t seq
    cdef readonly bytes data

cdef class Push:
    cdef readonly uint8_t flags
    cdef readonly bytes data

cdef class Ping:
    cdef readonly uint8_t flags
    cdef readonly uint32_t seq

cdef class Pong:
    cdef readonly uint8_t flags
    cdef readonly uint32_t seq

cdef class Hello:
    cdef readonly uint8_t flags
    cdef readonly uint8_t version
    cdef readonly list supported_encodings
    cdef readonly list supported_compressions

cdef class HelloAck:
    cdef readonly uint8_t flags
    cdef readonly uint32_t ping_interval
    cdef readonly bytes selected_encoding
    cdef readonly bytes selected_compression

cdef class GoAway:
    cdef readonly uint8_t flags
    cdef readonly uint16_t code
    cdef readonly bytes reason

cdef class Error:
    cdef readonly uint8_t flags
    cdef readonly uint16_t code
    cdef readonly uint32_t seq
    cdef readonly bytes data