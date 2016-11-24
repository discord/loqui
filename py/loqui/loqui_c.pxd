from libc.stdint cimport uint32_t, uint8_t

cdef extern from "../../c/buffer.h":
    ctypedef struct loqui_buffer_t:
        char *buf
        size_t length
        size_t allocated_size

    ctypedef struct loqui_decode_buffer_t:
        loqui_buffer_t loqui_buffer;
        uint8_t opcode;
        size_t data_size_remaining;
        size_t header_size;

cdef extern from "../../c/encoder.h":
    int loqui_append_hello(loqui_buffer_t *b, uint32_t ping_interval, uint32_t size, const char *data)
    int loqui_append_select_encoding(loqui_buffer_t *b, uint32_t size, const char *data)
    int loqui_append_ping(loqui_buffer_t *b, uint32_t seq)
    int loqui_append_pong(loqui_buffer_t *b, uint32_t seq)
    int loqui_append_request(loqui_buffer_t *b, uint32_t seq, uint32_t size, const char *data)
    int loqui_append_response(loqui_buffer_t *b, uint32_t seq, uint32_t size, const char *data)
    int loqui_append_push(loqui_buffer_t *b, uint32_t size, const char *data)
    int loqui_append_goaway(loqui_buffer_t *b, uint8_t code, uint32_t size, const char *data)
    int loqui_append_error(loqui_buffer_t *b, uint8_t code, uint32_t seq, uint32_t size, const char *data)

cdef extern from "../../c/decoder.h":
    loqui_decoder_status loqui_decoder_read_data(loqui_decode_buffer_t *pk, size_t size, const char *data, size_t* consumed)
    loqui_decoder_status loqui_decoder_reset(loqui_decode_buffer_t *pk)
    uint32_t loqui_get_seq(loqui_decode_buffer_t *pk)
    size_t loqui_get_data_payload_size(loqui_decode_buffer_t *pk)
    uint8_t loqui_get_version(loqui_decode_buffer_t *pk)
    uint8_t loqui_get_code(loqui_decode_buffer_t *pk)
    uint32_t loqui_get_ping_interval(loqui_decode_buffer_t *pk)

cdef extern from "../../c/constants.h":
    const unsigned char LOQUI_VERSION;
    const size_t LOQUI_DATA_SIZE_MAX;

    ctypedef enum loqui_opcodes:
        LOQUI_OP_HELLO
        LOQUI_OP_SELECT_ENCODING
        LOQUI_OP_PING
        LOQUI_OP_PONG
        LOQUI_OP_REQUEST
        LOQUI_OP_RESPONSE
        LOQUI_OP_PUSH
        LOQUI_OP_GOAWAY
        LOQUI_OP_ERROR

    ctypedef enum loqui_decoder_status:
        LOQUI_DECODE_NEEDS_MORE
        LOQUI_DECODE_COMPLETE
        LOQUI_DECODE_MEMORY_ERROR
        LOQUI_DECODE_INVALID_OPCODE
        LOQUI_DECODE_INVALID_SIZE
