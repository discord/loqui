#include <stddef.h>
#include <stdlib.h>
#include "sysdep.h"
#include "constants.h"
#include "buffer.h"
#include <limits.h>
#include <string.h>

#ifndef LOQUI_ENCODER_H__
#define LOQUI_ENCODER_H__

#define loqui_append(pk, buf, len) return loqui_buffer_write(pk, (const char *)buf, len)

static inline int loqui_append_hello(loqui_buffer_t *b, uint32_t ping_interval, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[size];
  buf[0] = LOQUI_OP_HELLO;
  buf[1] = LOQUI_VERSION;
  _loqui_store32(buf + 2, ping_interval);
  _loqui_store32(buf + 6, size);
  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_ping(loqui_buffer_t *b, uint32_t seq) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[1 + sizeof(uint32_t)];
  buf[0] = LOQUI_OP_PING;
  _loqui_store32(buf + 1, seq);
  loqui_append(b, buf, SIZE);
  #undef SIZE
}

static inline int loqui_append_pong(loqui_buffer_t *b, uint32_t seq) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_PONG;
  _loqui_store32(buf + 1, seq);
  loqui_append(b, buf, SIZE);
  #undef SIZE
}

static inline int loqui_append_request(loqui_buffer_t *b, uint32_t seq, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_REQUEST;
  _loqui_store32(buf + 1, seq);
  _loqui_store32(buf + 5, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_response(loqui_buffer_t *b, uint32_t seq, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_RESPONSE;
  _loqui_store32(buf + 1, seq);
  _loqui_store32(buf + 5, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_push(loqui_buffer_t *b, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_PUSH;
  _loqui_store32(buf + 1, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_select_encoding(loqui_buffer_t *b, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_SELECT_ENCODING;
  _loqui_store32(buf + 1, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_goaway(loqui_buffer_t *b, uint8_t code, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_GOAWAY;
  buf[1] = code;
  _loqui_store32(buf + 2, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  loqui_append(b, data, size);
}

static inline int loqui_append_error(loqui_buffer_t *b, uint8_t code, uint32_t seq, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = LOQUI_OP_GOAWAY;
  buf[1] = code;
  _loqui_store32(buf + 2, seq);
  _loqui_store32(buf + 6, size);

  int ret = loqui_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  if (size > 0) {
    loqui_append(b, data, size);
  }
  else {
    return 0;
  }
}

#undef loqui_append

#endif