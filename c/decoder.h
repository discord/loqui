#include <stddef.h>
#include <stdlib.h>
#include "sysdep.h"
#include "constants.h"
#include "buffer.h"
#include <limits.h>
#include <string.h>

#ifndef DRPC_DECODER_H__
#define DRPC_DECODER_H__

static inline size_t _drpc_get_header_size(uint8_t opcode) {
  switch (opcode) {
    case DRPC_OP_HELLO:
      return sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t);
    case DRPC_OP_PING:
    case DRPC_OP_PONG:
      return sizeof(uint32_t);
    case DRPC_OP_REQUEST:
    case DRPC_OP_RESPONSE:
      return sizeof(uint32_t) + sizeof(uint32_t);
    case DRPC_OP_PUSH:
    case DRPC_OP_SELECT_ENCODING:
      return sizeof(uint32_t);
    case DRPC_OP_GOAWAY:
      return sizeof(uint8_t) + sizeof(uint32_t);
    default:
      return 0;
  }
}

static inline size_t drpc_get_data_payload_size(drpc_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case DRPC_OP_REQUEST:
    case DRPC_OP_RESPONSE:
      return _drpc_load32(size_t, pk->drpc_buffer.buf + sizeof(uint32_t));
    case DRPC_OP_PUSH:
    case DRPC_OP_SELECT_ENCODING:
      return _drpc_load32(size_t, pk->drpc_buffer.buf);
    case DRPC_OP_GOAWAY:
      return _drpc_load32(size_t, pk->drpc_buffer.buf + sizeof(uint8_t));
    case DRPC_OP_HELLO:
      return _drpc_load32(size_t, pk->drpc_buffer.buf + sizeof(uint8_t) + sizeof(uint32_t));
    default:
      return 0;
  }
}

static inline size_t drpc_get_seq(drpc_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case DRPC_OP_REQUEST:
    case DRPC_OP_RESPONSE:
    case DRPC_OP_PING:
    case DRPC_OP_PONG:
      return _drpc_load32(size_t, pk->drpc_buffer.buf);
    default:
      return 0;
  }
}

static inline uint8_t drpc_get_version(drpc_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case DRPC_OP_HELLO:
      return (uint8_t) pk->drpc_buffer.buf[0];
    default:
      return 0;
  }
}
static inline uint8_t drpc_get_code(drpc_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case DRPC_OP_GOAWAY:
      return (uint8_t) pk->drpc_buffer.buf[0];
    default:
      return 0;
  }
}

static inline uint8_t drpc_get_ping_interval(drpc_decode_buffer_t* pk) {
  switch (pk->opcode) {
    case DRPC_OP_HELLO:
      return _drpc_load32(size_t, pk->drpc_buffer.buf + sizeof(uint8_t));
    default:
      return 0;
  }
}

static inline void drpc_decoder_reset(drpc_decode_buffer_t* pk) {
  pk->opcode = 0;
  pk->data_size_remaining = 0;
  pk->decode_complete = 0;
}

static inline drpc_decoder_status drpc_read_append_data(drpc_decode_buffer_t* pk,
                                                        size_t size,
                                                        const char* data,
                                                        size_t* consumed) {
  if (pk->opcode == 0) {
    return DRPC_DECODE_INVALID_OPCODE;
  }

  int rv;

  // we haven't had the chance to read the header yet.
  if (pk->drpc_buffer.length < pk->header_size) {
    // How many bytes remain to read the full header?
    size_t header_chunk_to_read = pk->header_size - pk->drpc_buffer.length;
    if (header_chunk_to_read > size) {
      header_chunk_to_read = size;
    }

    // Write that data to the buffer.
    rv = drpc_buffer_write(&pk->drpc_buffer, data, header_chunk_to_read);
    if (rv < 0) {
      return DRPC_DECODE_MEMORY_ERROR;
    }

    // Mark that we've consumed the data.
    *consumed += header_chunk_to_read;

    // We don't have the full header yet.
    if (pk->drpc_buffer.length < pk->header_size) {
      return DRPC_DECODE_NEEDS_MORE;
    }

    // We've consumed the full header, figure out if there's a data payload.
    pk->data_size_remaining = drpc_get_data_payload_size(pk);

    // The data payload is too big. Bail out before we allocate excessive memory to handle it.
    if (pk->data_size_remaining > DRPC_DATA_SIZE_MAX) {
      return DRPC_DECODE_INVALID_SIZE;
    }

    // We have a data payload! Pre-allocate a buffer that's big enough to hold the resulting data.
    if (pk->data_size_remaining > 0) {
      rv = drpc_buffer_ensure_size(&pk->drpc_buffer, pk->header_size + pk->data_size_remaining);
      if (rv < 0) {
        return DRPC_DECODE_MEMORY_ERROR;
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
    rv = drpc_buffer_write(&pk->drpc_buffer, data, bytes_to_consume);
    if (rv < 0) {
      return DRPC_DECODE_MEMORY_ERROR;
    }

    pk->data_size_remaining -= bytes_to_consume;
    *consumed += bytes_to_consume;
  }

  if (pk->data_size_remaining == 0) {
    pk->decode_complete = 1;
    return DRPC_DECODE_COMPLETE;
  }
  else {
    return DRPC_DECODE_NEEDS_MORE;
  }
}

static inline drpc_decoder_status drpc_read_new_data(drpc_decode_buffer_t* pk,
                                                     size_t size,
                                                     const char* data,
                                                     size_t* consumed) {
  uint8_t opcode = (uint8_t) data[0];
  size_t header_size = _drpc_get_header_size(opcode);
  if (header_size == 0) {
    return DRPC_DECODE_INVALID_OPCODE;
  }

  *consumed += 1;
  data = data + 1;
  size -= 1;

  pk->drpc_buffer.length = 0;
  pk->data_size_remaining = 0;
  pk->opcode = opcode;
  pk->header_size = header_size;

  if (size > 0) {
    return drpc_read_append_data(pk, size, data, consumed);
  }

  return DRPC_DECODE_NEEDS_MORE;
}

static inline drpc_decoder_status drpc_decoder_read_data(drpc_decode_buffer_t* pk,
                                                         size_t size,
                                                         const char* data,
                                                         size_t* consumed) {
  if (pk->decode_complete == 1) {
    return DRPC_DECODE_COMPLETE;
  }
  else if (pk->opcode == 0) {
    return drpc_read_new_data(pk, size, data, consumed);
  }
  else {
    return drpc_read_append_data(pk, size, data, consumed);
  }
}

#endif
