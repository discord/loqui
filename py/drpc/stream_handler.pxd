from libc.stdint cimport uint32_t, uint8_t
cimport drpc_c

cdef class DRPCStreamHandler:
    cdef uint32_t seq
    cdef drpc_c.drpc_decode_buffer_t decode_buffer
    cdef drpc_c.drpc_buffer_t write_buffer
    cdef size_t write_buffer_position

    cdef inline _reset_decode_buf(self)
    cdef _reset_or_compact_write_buf(self)
    cpdef uint32_t current_seq(self)
    cdef inline uint32_t next_seq(self)
    cpdef uint32_t send_ping(self) except 0
    cpdef uint32_t send_pong(self, uint32_t seq) except 0
    cpdef uint32_t send_request(self, bytes data) except 0
    cpdef uint32_t send_push(self, bytes data) except 0
    cpdef uint32_t send_select_encoding(self, bytes data) except 0
    cpdef uint32_t send_response(self, uint32_t seq, bytes data) except 0
    cpdef uint32_t send_hello(self, uint32_t ping_interval, list available_encodings) except 0
    cpdef uint32_t send_error(self, uint8_t code, uint32_t seq, bytes data) except 0
    cpdef uint32_t send_goaway(self, uint8_t code, bytes reason) except 0
    cpdef size_t write_buffer_len(self)
    cpdef bytes write_buffer_get_bytes(self, size_t length, bint consume=?)
    cpdef size_t write_buffer_consume_bytes(self, size_t length)
    cpdef list on_bytes_received(self, bytes data)
    cdef object _consume_decode_buffer(self)