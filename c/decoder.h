#include <stddef.h>
#include <stdlib.h>
#include "sysdep.h"
#include "constants.h"
#include "buffer.h"
#include <limits.h>
#include <string.h>

#ifndef LOQUI_DECODER_H__
#define LOQUI_DECODER_H__

static inline size_t _loqui_get_header_size(uint8_t opcode) {
  switch (opcode) {
    case LOQUI_OP_HELLO:
      return sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t);
    case LOQUI_OP_HELLO_ACK:
      return sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t);
    case LOQUI_OP_ERROR:
      return sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint16_t) + sizeof(uint32_t);
    case LOQUI_OP_PING:
    case LOQUI_OP_PONG:
      return sizeof(uint8_t) + sizeof(uint32_t);
    case LOQUI_OP_REQUEST:
    case LOQUI_OP_RESPONSE:
      return sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t);
    case LOQUI_OP_PUSH:
      return sizeof(uint8_t) + sizeof(uint32_t);
    case LOQUI_OP_GOAWAY:
      return sizeof(uint8_t) + sizeof(uint16_t) + sizeof(uint32_t);
    default:
      return 0;
  }
}

static inline uint8_t loqui_get_flags(loqui_decode_buffer_t* pk) {
    return (uint8_t) pk->loqui_buffer.buf[0];
}

static inline size_t loqui_get_data_payload_size(loqui_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case LOQUI_OP_REQUEST:
    case LOQUI_OP_RESPONSE:
    case LOQUI_OP_HELLO_ACK:
      return _loqui_load32(size_t, pk->loqui_buffer.buf + sizeof(uint8_t) + sizeof(uint32_t));
    case LOQUI_OP_PUSH:
      return _loqui_load32(size_t, pk->loqui_buffer.buf + sizeof(uint8_t));
    case LOQUI_OP_GOAWAY:
      return _loqui_load32(size_t, pk->loqui_buffer.buf + sizeof(uint8_t) + sizeof(uint8_t));
    case LOQUI_OP_ERROR:
      return _loqui_load32(size_t, pk->loqui_buffer.buf + sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint16_t));
    case LOQUI_OP_HELLO:
      return _loqui_load32(size_t, pk->loqui_buffer.buf + sizeof(uint8_t) + sizeof(uint8_t));
    default:
      return 0;
  }
}

static inline uint32_t loqui_get_seq(loqui_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case LOQUI_OP_REQUEST:
    case LOQUI_OP_RESPONSE:
    case LOQUI_OP_PING:
    case LOQUI_OP_PONG:
    case LOQUI_OP_ERROR:
      return _loqui_load32(uint32_t, pk->loqui_buffer.buf + sizeof(uint8_t));
    default:
      return 0;
  }
}

static inline uint8_t loqui_get_version(loqui_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case LOQUI_OP_HELLO:
      return (uint8_t) pk->loqui_buffer.buf[sizeof(uint8_t)];
    default:
      return 0;
  }
}

static inline uint16_t loqui_get_code(loqui_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case LOQUI_OP_GOAWAY:
      return _loqui_load16(uint16_t, pk->loqui_buffer.buf + sizeof(uint8_t));
    case LOQUI_OP_ERROR:
      return _loqui_load16(uint16_t, pk->loqui_buffer.buf + sizeof(uint8_t) + sizeof(uint32_t));
    default:
      return 0;
  }
}

static inline uint32_t loqui_get_ping_interval(loqui_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case LOQUI_OP_HELLO_ACK:
      return _loqui_load32(uint32_t, pk->loqui_buffer.buf + sizeof(uint8_t));
    default:
      return 0;
  }
}

static inline void loqui_decoder_reset(loqui_decode_buffer_t* pk) {
  pk->opcode = 0;
  pk->data_size_remaining = 0;
  pk->decode_complete = 0;
}

static inline loqui_decoder_status loqui_read_append_data(loqui_decode_buffer_t* pk,
                                                        size_t size,
                                                        const char* data,
                                                        size_t* consumed) {
  if (pk->opcode == 0) {
    return LOQUI_DECODE_INVALID_OPCODE;
  }

  int rv;

  // we haven't had the chance to read the header yet.
  if (pk->loqui_buffer.length < pk->header_size) {
    // How many bytes remain to read the full header?
    size_t header_chunk_to_read = pk->header_size - pk->loqui_buffer.length;
    if (header_chunk_to_read > size) {
      header_chunk_to_read = size;
    }

    // Write that data to the buffer.
    rv = loqui_buffer_write(&pk->loqui_buffer, data, header_chunk_to_read);
    if (rv < 0) {
      return LOQUI_DECODE_MEMORY_ERROR;
    }

    // Mark that we've consumed the data.
    *consumed += header_chunk_to_read;

    // We don't have the full header yet.
    if (pk->loqui_buffer.length < pk->header_size) {
      return LOQUI_DECODE_NEEDS_MORE;
    }

    // We've consumed the full header, figure out if there's a data payload.
    pk->data_size_remaining = loqui_get_data_payload_size(pk);

    // The data payload is too big. Bail out before we allocate excessive memory to handle it.
    if (pk->data_size_remaining > LOQUI_DATA_SIZE_MAX) {
      return LOQUI_DECODE_INVALID_SIZE;
    }

    // We have a data payload! Pre-allocate a buffer that's big enough to hold the resulting data.
    if (pk->data_size_remaining > 0) {
      rv = loqui_buffer_ensure_size(&pk->loqui_buffer, pk->header_size + pk->data_size_remaining);
      if (rv < 0) {
        return LOQUI_DECODE_MEMORY_ERROR;
      }
    }

    // Move the position in the data to where the payload is.
    data = data + header_chunk_to_read;
    size -= header_chunk_to_read;
  }

  // Do we have bytes to consume?
  size_t bytes_to_consume = pk->data_size_remaining;
  // Constrain it to the size of the remaining buffer.
  if (bytes_to_consume > size) {
    bytes_to_consume = size;
  }

  // If we have data to consume.
  if (bytes_to_consume > 0) {
    rv = loqui_buffer_write(&pk->loqui_buffer, data, bytes_to_consume);
    if (rv < 0) {
      return LOQUI_DECODE_MEMORY_ERROR;
    }

    pk->data_size_remaining -= bytes_to_consume;
    *consumed += bytes_to_consume;
  }

  if (pk->data_size_remaining == 0) {
    pk->decode_complete = 1;
    return LOQUI_DECODE_COMPLETE;
  }
  else {
    return LOQUI_DECODE_NEEDS_MORE;
  }
}

static inline loqui_decoder_status loqui_read_new_data(loqui_decode_buffer_t* pk,
                                                     size_t size,
                                                     const char* data,
                                                     size_t* consumed) {
  uint8_t opcode = (uint8_t) data[0];
  size_t header_size = _loqui_get_header_size(opcode);
  if (header_size == 0) {
    return LOQUI_DECODE_INVALID_OPCODE;
  }

  *consumed += 1;
  data = data + 1;
  size -= 1;

  pk->loqui_buffer.length = 0;
  pk->data_size_remaining = 0;
  pk->opcode = opcode;
  pk->header_size = header_size;

  if (size > 0) {
    return loqui_read_append_data(pk, size, data, consumed);
  }

  return LOQUI_DECODE_NEEDS_MORE;
}

static inline loqui_decoder_status loqui_decoder_read_data(loqui_decode_buffer_t* pk,
                                                         size_t size,
                                                         const char* data,
                                                         size_t* consumed) {
  if (pk->decode_complete == 1) {
    return LOQUI_DECODE_COMPLETE;
  }
  else if (pk->opcode == 0) {
    return loqui_read_new_data(pk, size, data, consumed);
  }
  else {
    return loqui_read_append_data(pk, size, data, consumed);
  }
}

#endif
