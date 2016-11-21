from cpython cimport *
from libc.stdlib cimport malloc, free
from libc.string cimport memcpy
from libc.stdint cimport uint32_t, uint8_t

cdef size_t BIG_BUF_SIZE = 1024 * 1024 * 2
cdef size_t INITIAL_BUFFER_SIZE = 1024 * 217
# Todo move to Header
cdef uint32_t SEQ_MAX = (2 ** 32) - 2

cimport drpc_c

cdef class DRPCStream:
    cdef int seq
    cdef bytes data


cdef inline void _ensure_buffer(drpc_c.drpc_buffer_t* drpc_buffer):
    if drpc_buffer.buf != NULL:
        drpc_buffer.length = 0

    else:
        drpc_buffer.buf = <char*> malloc(INITIAL_BUFFER_SIZE)
        if drpc_buffer.buf == NULL:
            raise MemoryError('Unable to allocate buffer')

        drpc_buffer.allocated_size = INITIAL_BUFFER_SIZE
        drpc_buffer.length = 0

cdef inline void _free_big_buffer(drpc_c.drpc_buffer_t* drpc_buffer):
    if drpc_buffer.allocated_size >= BIG_BUF_SIZE:
        free(drpc_buffer.buf)
        drpc_buffer.buf = NULL
        drpc_buffer.length = 0
        drpc_buffer.allocated_size = 0

    # It's not too big, so we can reset the length to 0.
    else:
        drpc_buffer.length = 0

cdef inline bytes _get_bytes_from_decode_buffer(drpc_c.drpc_decode_buffer_t* decode_buffer):
    cdef char* buf = decode_buffer.drpc_buffer.buf + drpc_c.drpc_get_required_initial_buffer_size(decode_buffer)
    cdef size_t size = drpc_c.drpc_get_data_payload_size(decode_buffer)
    if size == 0:
        return None

    # this creates a copy into python world.
    return PyBytes_FromStringAndSize(buf, size)

cdef class DRPCStreamHandler:
    cdef bint is_client
    cdef uint32_t seq
    cdef drpc_c.drpc_decode_buffer_t decode_buffer
    cdef drpc_c.drpc_buffer_t write_buffer
    cdef size_t write_buffer_position

    def __cinit__(self, bint client_mode=True):
        self.seq = 0
        self.write_buffer_position = 0
        self.is_client = client_mode

        self.write_buffer.buf = NULL
        self.decode_buffer.drpc_buffer.buf = NULL

        _ensure_buffer(&self.write_buffer)
        _ensure_buffer(&self.decode_buffer.drpc_buffer)

    cdef inline _reset_decode_buf(self):
        self.decode_buffer.opcode = 0
        _free_big_buffer(&self.decode_buffer.drpc_buffer)
        _ensure_buffer(&self.decode_buffer.drpc_buffer)

    cdef _reset_or_compact_write_buf(self):
        """
        If the write buffer is larger than `BIG_BUF_SIZE`, free it, so that writing large data does not hold onto
        the big buffer. Otherwise, try and compact the buffer.
        """
        if self.write_buffer_position == self.write_buffer.length:
            _free_big_buffer(&self.write_buffer)
            _ensure_buffer(&self.write_buffer)
            self.write_buffer_position = 0

        elif self.write_buffer.length > self.write_buffer_position > self.write_buffer.allocated_size / 2:
            memcpy(
                self.write_buffer.buf,
                self.write_buffer.buf + self.write_buffer_position,
                self.write_buffer.length - self.write_buffer_position
            )

            self.write_buffer_position -= self.write_buffer_position
            self.write_buffer.length -= self.write_buffer_position

    def __dealloc__(self):
        if self.write_buffer.buf != NULL:
            free(self.write_buffer.buf)

        if self.decode_buffer.drpc_buffer.buf != NULL:
            free(self.decode_buffer.drpc_buffer.buf)

    cpdef uint32_t current_seq(self):
        return self.seq

    cdef inline uint32_t _next_seq(self):
        cdef uint32_t next_seq
        self.seq += 1
        next_seq = self.seq
        if next_seq >= SEQ_MAX:
            self.seq = 0
            next_seq = 0

        return next_seq

    cpdef uint32_t send_ping(self):
        cdef int rv
        cdef uint32_t seq = self._next_seq()
        rv = drpc_c.drpc_append_ping(&self.write_buffer, seq)
        if rv < 0:
            raise MemoryError('not enough memory to fulfill buffer')

        return seq

    cpdef send_pong(self, uint32_t seq):
        cdef int rv
        rv = drpc_c.drpc_append_pong(&self.write_buffer, seq)
        if rv < 0:
            raise MemoryError('not enough memory to fulfill buffer')

    cpdef uint32_t send_request(self, bytes data):
        cdef int rv
        cdef uint32_t seq = self._next_seq()
        cdef char *buffer
        cdef size_t size

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError('data is not bytes!')

        rv = drpc_c.drpc_append_request(&self.write_buffer, seq, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError('not enough memory to fulfill buffer')

        return seq

    cpdef void send_response(self, uint32_t seq, bytes data):
        cdef int rv
        cdef char *buffer
        cdef size_t size

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError('data is not bytes!')

        rv = drpc_c.drpc_append_request(&self.write_buffer, seq, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError('not enough memory to fulfill buffer')


    cpdef size_t write_buffer_len(self):
        return self.write_buffer.length - self.write_buffer_position

    cpdef bytes write_buffer_get_bytes(self, size_t length):
        """
        Returns a bytes object that has at least n bytes from the write buffer.

        :param length: The number of bytes to get from the write buffer.
        """

        cdef size_t buffer_len_remaining = self.write_buffer_len()
        if length > buffer_len_remaining:
            length = buffer_len_remaining

        if length == 0:
            return None

        cdef bytes write_buffer = PyBytes_FromStringAndSize(self.write_buffer.buf + self.write_buffer_position, length)
        self.write_buffer_position += length
        self._reset_or_compact_write_buf()
        return write_buffer

    cpdef list on_bytes_received(self, bytes data):
        cdef size_t size = PyBytes_Size(data)
        cdef char* buf = PyBytes_AS_STRING(data)
        cdef size_t consumed = 0
        cdef drpc_c.drpc_decoder_status decoder_status

        cdef list received_payloads = []
        while size > 0:
            decoder_status = drpc_c.drpc_decoder_read_data(&self.decode_buffer, size, buf, &consumed)
            if decoder_status < 0:
                self._reset_decode_buf()
                raise Exception('The decoder failed with status %s' % decoder_status)

            if decoder_status == drpc_c.DRPC_DECODE_NEEDS_MORE:
                break

            elif decoder_status == drpc_c.DRPC_DECODE_COMPLETE:
                received_payloads.append(self._consume_decode_buffer())

            else:
                raise Exception('Unhandled decoder status %s' % decoder_status)

            size -= consumed
            buf += consumed
            consumed = 0

        return received_payloads

    cdef object _consume_decode_buffer(self):
        cdef uint8_t opcode = self.decode_buffer.opcode
        cdef object response

        if opcode == drpc_c.DRPC_OP_RESPONSE:
            response = Response(
                seq=drpc_c.drpc_get_seq(&self.decode_buffer),
                data=_get_bytes_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == drpc_c.DRPC_OP_REQUEST:
            response = Request(
                seq=drpc_c.drpc_get_seq(&self.decode_buffer),
                data=_get_bytes_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == drpc_c.DRPC_OP_PUSH:
            response = Push(
                data=_get_bytes_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == drpc_c.DRPC_OP_PING:
            response = Ping(
                seq=drpc_c.drpc_get_seq(&self.decode_buffer)
            )
            self.send_pong(response.seq)

        elif opcode == drpc_c.DRPC_OP_PONG:
            response = Pong(
                seq=drpc_c.drpc_get_seq(&self.decode_buffer),
            )

        elif opcode == drpc_c.DRPC_OP_GOAWAY:
            response = GoAway(
                code=drpc_c.drpc_get_code(&self.decode_buffer),
                data=_get_bytes_from_decode_buffer(&self.decode_buffer)
            )

        self._reset_decode_buf()
        return response

cdef class Response:
    cdef readonly uint32_t seq
    cdef readonly bytes data

    def __cinit__(self, uint32_t seq, bytes data):
        self.seq = seq
        self.data = data

cdef class Request:
    cdef readonly uint32_t seq
    cdef readonly bytes data

    def __cinit__(self, uint32_t seq, bytes data):
        self.seq = seq
        self.data = data


cdef class Push:
    cdef readonly bytes data

    def __cinit__(self, bytes data):
        self.data = data

cdef class Ping:
    cdef readonly uint32_t seq

    def __cinit__(self, uint32_t seq):
        self.seq = seq


cdef class Pong:
    cdef readonly uint32_t seq

    def __cinit__(self, uint32_t seq):
        self.seq = seq

cdef class Hello:
    cdef readonly uint8_t seq
    cdef readonly uint32_t ping_interval

    def __cinit__(self, uint32_t seq, uint32_t ping_interval):
        self.seq = seq
        self.ping_interval = ping_interval

cdef class GoAway:
    cdef readonly uint8_t code
    cdef readonly bytes data

    def __cinit__(self, uint8_t code, bytes data):
        self.code = code
        self.data = data