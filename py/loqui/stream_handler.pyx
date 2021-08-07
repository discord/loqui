from cpython cimport *
from libc.stdlib cimport malloc, free
from libc.string cimport memcpy
from libc.stdint cimport uint32_t, uint8_t, uint16_t

from exceptions import LoquiDecoderError
cimport loqui_c
cimport opcodes

cdef size_t BIG_BUF_SIZE = 1024 * 1024 * 2
cdef size_t INITIAL_BUFFER_SIZE = 1024 * 512
# Todo move to Header
cdef uint32_t SEQ_MAX = (2 ** 32) - 2


cdef inline void _reset_buffer(loqui_c.loqui_buffer_t* loqui_buffer):
    """
    Ensures the buffer is in a reset state.
    """
    if loqui_buffer.buf != NULL:
        loqui_buffer.length = 0

    else:
        loqui_buffer.buf = <char*> malloc(INITIAL_BUFFER_SIZE)
        if loqui_buffer.buf == NULL:
            raise MemoryError('Unable to allocate buffer')

        loqui_buffer.allocated_size = INITIAL_BUFFER_SIZE
        loqui_buffer.length = 0

cdef inline void _free_big_buffer(loqui_c.loqui_buffer_t* loqui_buffer):
    """
    Frees the buffer if it's grown over `BIG_BUF_SIZE`.
    """
    if loqui_buffer.allocated_size >= BIG_BUF_SIZE:
        free(loqui_buffer.buf)
        loqui_buffer.buf = NULL
        loqui_buffer.length = 0
        loqui_buffer.allocated_size = 0

    # It's not too big, so we can reset the length to 0.
    else:
        loqui_buffer.length = 0

cdef inline bytes _get_payload_from_decode_buffer(loqui_c.loqui_decode_buffer_t* decode_buffer):
    """
    Get the payload part of the decode buffer. Copies the data after the headers in the buffer into a PyBytes object.
    """
    cdef char* buf = decode_buffer.loqui_buffer.buf + decode_buffer.header_size
    cdef size_t size = loqui_c.loqui_get_data_payload_size(decode_buffer)
    if size == 0:
        return None

    # this creates a copy into python world.
    return PyBytes_FromStringAndSize(buf, size)

cdef class LoquiStreamHandler:
    """
    The stream handler handles reading and parsing data, and maintaining a write buffer for events through the RPC.
    The handler does not track incoming or outgoing events, and does not validate the ordering of these events.
    It's expected that a session handler will wrap the stream handler, and perform those actions. The stream handler
    mainly exists to consume bytes and produce events, and take events and return bytes to be written. The stream
    handler however will generate sequences for the relevant events, and return those sequences in their respective
    `send_` functions.
    """
    def __cinit__(self):
        self.seq = 0
        self.write_buffer_position = 0

        _reset_buffer(&self.write_buffer)
        _reset_buffer(&self.decode_buffer.loqui_buffer)

    def __dealloc__(self):
        if self.write_buffer.buf != NULL:
            free(self.write_buffer.buf)

        if self.decode_buffer.loqui_buffer.buf != NULL:
            free(self.decode_buffer.loqui_buffer.buf)

    cpdef uint32_t current_seq(self):
        """
        Returns the latest seq issued.
        """
        return self.seq

    cdef inline uint32_t next_seq(self):
        """
        Returns the next sequence to be issued, and increments the local seq counter. Handles seq overflow by
        restarting at 0.
        """
        cdef uint32_t next_seq
        self.seq += 1
        next_seq = self.seq
        if next_seq >= SEQ_MAX:
            self.seq = 0
            next_seq = 0

        return next_seq

    cpdef uint32_t send_ping(self, uint8_t flags) except 0:
        """
        Enqueue a ping to be sent.
        :param flags: Ping flags
        :return: Returns the seq of the ping being sent.
        """
        cdef int rv
        cdef uint32_t seq = self.next_seq()
        rv = loqui_c.loqui_append_ping(&self.write_buffer, flags, seq)
        if rv < 0:
            raise MemoryError()

        return seq

    cpdef uint32_t send_pong(self, uint8_t flags, uint32_t seq) except 0:
        """
        Enqueues a pong to be sent.
        :param flags: Pong flags
        :param seq: The seq to pong.
        :return:
        """
        cdef int rv
        rv = loqui_c.loqui_append_pong(&self.write_buffer, flags, seq)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_request(self, uint8_t flags, bytes data) except 0:
        """
        Enqueues a request type message to be sent, with the given bytes `data` as the payload.
        :param flags: Request flags
        :param data: The data to send
        :return: The seq of the request being sent.
        """
        cdef int rv
        cdef uint32_t seq = self.next_seq()
        cdef char *buffer
        cdef size_t size

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError()

        rv = loqui_c.loqui_append_request(&self.write_buffer, flags, seq, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return seq

    cpdef uint32_t send_push(self, uint8_t flags, bytes data) except 0:
        """
        Enqueues a push type message to be sent, with the given bytes `data` as the payload.
        :param flags: Push flags
        :param data: The data to send
        """
        cdef int rv
        cdef char *buffer
        cdef size_t size

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError()

        rv = loqui_c.loqui_append_push(&self.write_buffer, flags, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_hello(self, uint8_t flags, list supported_encodings, list supported_compressors) except 0:
        """
        Enqueues a hello type message to be sent, with the given supported encodings and compressors

        :param flags: Hello flags.
        :param supported_encodings: Available encodings that the client understands
        :param supported_compressors: Available compressors that the client understands
        """
        cdef int rv
        cdef char *buffer
        cdef size_t size
        cdef bytes data = bytes(b'%s|%s' % (
            b','.join([bytes(b) for b in supported_encodings]),
            b','.join([bytes(b) for b in supported_compressors])
        ))

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError()

        rv = loqui_c.loqui_append_hello(&self.write_buffer, flags, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_hello_ack(self, uint8_t flags, uint32_t ping_interval,  bytes selected_encoding, bytes selected_compressor) except 0:
        """
        Acknowledges a hello, and handshakes - establishing a loqui socket session.

        :param flags: Hello Ack flags.
        :param ping_interval: How often in (ms) the client should be expected to ping
        :param selected_encoding: The encoding the server selected.
        :param selected_compressor: The compressor the server selected
        """
        cdef int rv
        cdef char *buffer
        cdef size_t size
        cdef bytes data = bytes(b'%s|%s' % (
            selected_encoding,
            selected_compressor or b''
        ))

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError()

        rv = loqui_c.loqui_append_hello_ack(&self.write_buffer, flags, ping_interval, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_response(self, uint8_t flags, uint32_t seq, bytes data) except 0:
        """
        Enqueues a response type message to be sent, with the given seq and bytes `data` as the payload.
        This function does not validate if the given seq is in-flight. It's assumed that the client will handle
        duplicate responses correctly.

        :param flags: Response flags.
        :param seq: The seq to reply to.
        :param data: The data to respond with.
        """
        cdef int rv
        cdef char *buffer
        cdef size_t size

        rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
        if rv < 0:
            raise TypeError()

        rv = loqui_c.loqui_append_response(&self.write_buffer, flags, seq, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_error(self, uint8_t flags, uint16_t code, uint32_t seq, bytes data) except 0:
        """
        Enqueues a response type message to be sent, with the given seq and bytes `data` as the payload.
        This function does not validate if the given seq is in-flight. It's assumed that the client will handle
        duplicate responses correctly.

        :param flags: Error flags.
        :param code: The error code
        :param seq: The seq to reply to.
        :param data: The data to respond with.
        """
        cdef int rv
        cdef char *buffer = NULL
        cdef size_t size = 0

        if data:
            rv = PyBytes_AsStringAndSize(data, &buffer, <Py_ssize_t*> &size)
            if rv < 0:
                raise TypeError()

        rv = loqui_c.loqui_append_error(&self.write_buffer, flags, code, seq, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef uint32_t send_goaway(self, uint8_t flags, uint16_t code, bytes reason) except 0:
        """
        Enqueues a goaway message to be sent - letting the remote end know that a connection termination is imminent.

        :param flags: Goaway flags
        :param code: The go away code
        :param reason: A human readable string describing the reason for going away.
        """
        cdef int rv
        cdef char *buffer = NULL
        cdef size_t size = 0

        if reason:
            rv = PyBytes_AsStringAndSize(reason, &buffer, <Py_ssize_t*> &size)
            if rv < 0:
                raise TypeError()

        rv = loqui_c.loqui_append_goaway(&self.write_buffer, flags, code, size, <const char*> buffer)
        if rv < 0:
            raise MemoryError()

        return 1

    cpdef size_t write_buffer_len(self):
        """
        Gets the length of the write buffer - if non-zero, it means that we have data to write.
        """
        return self.write_buffer.length - self.write_buffer_position

    cpdef bytes write_buffer_get_bytes(self, size_t length, bint consume=True):
        """
        Returns a bytes object that has at least n bytes from the write buffer. This function removes the data
        from the buffer. It's assumed that the caller will hold onto this data until it's successfully sent.

        :param length: The number of bytes to get from the write buffer.
        """

        cdef size_t buffer_len_remaining = self.write_buffer_len()
        if length > buffer_len_remaining:
            length = buffer_len_remaining

        if length == 0:
            return None

        cdef bytes write_buffer = PyBytes_FromStringAndSize(self.write_buffer.buf + self.write_buffer_position, length)
        if consume:
            self.write_buffer_position += length
            self._reset_or_compact_write_buf()

        return write_buffer

    cpdef size_t write_buffer_consume_bytes(self, size_t length):
        cdef size_t buffer_len_remaining = self.write_buffer_len()
        if length > buffer_len_remaining:
            length = buffer_len_remaining

        self.write_buffer_position += length
        self._reset_or_compact_write_buf()
        return buffer_len_remaining - length

    cpdef list on_bytes_received(self, bytes data):
        """
        Call this function when bytes are received from the remote end. It will return a list of objects that have
        been parsed. Attempt to send data after calling this function, as it may have generated data to be written.

        :param data: The data received from the remote end.
        :return: List of objects that have been received.
        """
        cdef size_t size = PyBytes_Size(data)
        cdef char* buf = PyBytes_AS_STRING(data)
        cdef size_t consumed = 0
        cdef loqui_c.loqui_decoder_status decoder_status

        cdef list received_payloads = []
        while size > 0:
            decoder_status = loqui_c.loqui_decoder_read_data(&self.decode_buffer, size, buf, &consumed)
            if decoder_status < 0:
                self._reset_decode_buf()
                raise LoquiDecoderError('The decoder failed with status %s' % decoder_status)

            if decoder_status == loqui_c.LOQUI_DECODE_NEEDS_MORE:
                break

            elif decoder_status == loqui_c.LOQUI_DECODE_COMPLETE:
                received_payloads.append(self._consume_decode_buffer())

            else:
                raise LoquiDecoderError('Unhandled decoder status %s' % decoder_status)

            size -= consumed
            buf += consumed
            consumed = 0

        return received_payloads

    cdef object _consume_decode_buffer(self):
        """
        Consumes the decode buffer, returning an opcode Event. Call this once the decoder returns LOQUI_DECODE_COMPLETE.
        This function must be called before attempting to decode more data.
        """
        cdef uint8_t opcode = self.decode_buffer.opcode
        cdef object response

        if opcode == loqui_c.LOQUI_OP_RESPONSE:
            response = opcodes.Response(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_seq(&self.decode_buffer),
                _get_payload_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == loqui_c.LOQUI_OP_REQUEST:
            response = opcodes.Request(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_seq(&self.decode_buffer),
                _get_payload_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == loqui_c.LOQUI_OP_PUSH:
            response = opcodes.Push(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                _get_payload_from_decode_buffer(&self.decode_buffer)
            )

        elif opcode == loqui_c.LOQUI_OP_PING:
            response = opcodes.Ping(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_seq(&self.decode_buffer)
            )
            self.send_pong(response.flags, response.seq)

        elif opcode == loqui_c.LOQUI_OP_PONG:
            response = opcodes.Pong(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_seq(&self.decode_buffer),
            )

        elif opcode == loqui_c.LOQUI_OP_GOAWAY:
            response = opcodes.GoAway(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_code(&self.decode_buffer),
                _get_payload_from_decode_buffer(&self.decode_buffer)
            )
        elif opcode == loqui_c.LOQUI_OP_HELLO:
            payload = _get_payload_from_decode_buffer(&self.decode_buffer)
            supported_encodings, supported_compressions = payload.split(b'|')

            response = opcodes.Hello(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_version(&self.decode_buffer),
                supported_encodings.split(b','),
                supported_compressions.split(b',')
            )

        elif opcode == loqui_c.LOQUI_OP_HELLO_ACK:
            payload = _get_payload_from_decode_buffer(&self.decode_buffer)
            selected_encoding, selected_compression = payload.split(b'|')

            response = opcodes.HelloAck(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_ping_interval(&self.decode_buffer),
                selected_encoding,
                selected_compression
            )

        elif opcode == loqui_c.LOQUI_OP_ERROR:
            response = opcodes.Error(
                loqui_c.loqui_get_flags(&self.decode_buffer),
                loqui_c.loqui_get_code(&self.decode_buffer),
                loqui_c.loqui_get_seq(&self.decode_buffer),
                _get_payload_from_decode_buffer(&self.decode_buffer)
            )

        self._reset_decode_buf()
        return response

    cdef inline _reset_decode_buf(self):
        """
        Resets the decode buffer, re-allocating the buffer if it's grown too big - otherwise, reuse the same
        buffer.
        """
        loqui_c.loqui_decoder_reset(&self.decode_buffer)
        _free_big_buffer(&self.decode_buffer.loqui_buffer)
        _reset_buffer(&self.decode_buffer.loqui_buffer)

    cdef _reset_or_compact_write_buf(self):
        """
        If the write buffer is larger than `BIG_BUF_SIZE`, free it, so that writing large data does not hold onto
        the big buffer. Otherwise, try and compact the buffer - by moving bytes towards the end of the buffer
        to the beginning, so that it can continue to be written to without needing a re-allocation.
        """

        # The buffer has been fully written. Shrink it if needed, otherwise reset the position to the beginning.
        if self.write_buffer_position == self.write_buffer.length:
            _free_big_buffer(&self.write_buffer)
            _reset_buffer(&self.write_buffer)
            self.write_buffer_position = 0

        # Attempt to compact the buffer.
        elif self.write_buffer.length > self.write_buffer_position > self.write_buffer.allocated_size / 2:
            memcpy(
                self.write_buffer.buf,
                self.write_buffer.buf + self.write_buffer_position,
                self.write_buffer.length - self.write_buffer_position
            )

            self.write_buffer.length -= self.write_buffer_position
            self.write_buffer_position = 0
