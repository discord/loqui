from libc.stdint cimport uint32_t, uint8_t

cdef extern from "../../c/buffer.h":
    ctypedef struct drpc_buffer_t:
        char *buf
        size_t length
        size_t allocated_size

    ctypedef struct drpc_decode_buffer_t:
        drpc_buffer_t drpc_buffer;
        uint8_t opcode;
        size_t data_size_remaining;
        size_t header_size;

cdef extern from "../../c/encoder.h":
    int drpc_append_hello(drpc_buffer_t *b, uint32_t ping_interval, uint32_t size, const char *data)
    int drpc_append_select_encoding(drpc_buffer_t *b, uint32_t size, const char *data)
    int drpc_append_ping(drpc_buffer_t *b, uint32_t seq)
    int drpc_append_pong(drpc_buffer_t *b, uint32_t seq)
    int drpc_append_request(drpc_buffer_t *b, uint32_t seq, uint32_t size, const char *data)
    int drpc_append_response(drpc_buffer_t *b, uint32_t seq, uint32_t size, const char *data)
    int drpc_append_push(drpc_buffer_t *b, uint32_t size, const char *data)
    int drpc_append_goaway(drpc_buffer_t *b, uint8_t code, uint32_t size, const char *data)
    int drpc_append_error(drpc_buffer_t *b, uint8_t code, uint32_t seq, uint32_t size, const char *data)

cdef extern from "../../c/decoder.h":
    drpc_decoder_status drpc_decoder_read_data(drpc_decode_buffer_t *pk, size_t size, const char *data, size_t* consumed)
    drpc_decoder_status drpc_decoder_reset(drpc_decode_buffer_t *pk)
    uint32_t drpc_get_seq(drpc_decode_buffer_t *pk)
    size_t drpc_get_data_payload_size(drpc_decode_buffer_t *pk)
    uint8_t drpc_get_version(drpc_decode_buffer_t *pk)
    uint8_t drpc_get_code(drpc_decode_buffer_t *pk)
    uint32_t drpc_get_ping_interval(drpc_decode_buffer_t *pk)

cdef extern from "../../c/constants.h":
    const unsigned char DRPC_VERSION;
    const size_t DRPC_DATA_SIZE_MAX;

    ctypedef enum drpc_opcodes:
        DRPC_OP_HELLO
        DRPC_OP_SELECT_ENCODING
        DRPC_OP_PING
        DRPC_OP_PONG
        DRPC_OP_REQUEST
        DRPC_OP_RESPONSE
        DRPC_OP_PUSH
        DRPC_OP_GOAWAY
        DRPC_OP_ERROR

    ctypedef enum drpc_decoder_status:
        DRPC_DECODE_NEEDS_MORE
        DRPC_DECODE_COMPLETE
        DRPC_DECODE_MEMORY_ERROR
        DRPC_DECODE_INVALID_OPCODE
        DRPC_DECODE_INVALID_SIZE
