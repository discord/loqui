#include <stddef.h>
#include <stdlib.h>
#include "sysdep.h"
#include "constants.h"
#include "buffer.h"
#include <limits.h>
#include <string.h>

#ifndef DRPC_ENCODER_H__
#define DRPC_ENCODER_H__

#define drpc_append(pk, buf, len) return drpc_buffer_write(pk, (const char *)buf, len)

static inline int drpc_append_hello(drpc_buffer_t *b, uint32_t ping_interval, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[size];
  buf[0] = DRPC_OP_HELLO;
  buf[1] = DRPC_VERSION;
  _drpc_store32(buf + 2, ping_interval);
  _drpc_store32(buf + 6, size);
  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

static inline int drpc_append_ping(drpc_buffer_t *b, uint32_t seq) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[1 + sizeof(uint32_t)];
  buf[0] = DRPC_OP_PING;
  _drpc_store32(buf + 1, seq);
  drpc_append(b, buf, SIZE);
  #undef SIZE
}

static inline int drpc_append_pong(drpc_buffer_t *b, uint32_t seq) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_PONG;
  _drpc_store32(buf + 1, seq);
  drpc_append(b, buf, SIZE);
  #undef SIZE
}

static inline int drpc_append_request(drpc_buffer_t *b, uint32_t seq, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_REQUEST;
  _drpc_store32(buf + 1, seq);
  _drpc_store32(buf + 5, size);

  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

static inline int drpc_append_response(drpc_buffer_t *b, uint32_t seq, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_RESPONSE;
  _drpc_store32(buf + 1, seq);
  _drpc_store32(buf + 5, size);

  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

static inline int drpc_append_push(drpc_buffer_t *b, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_PUSH;
  _drpc_store32(buf + 1, size);

  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

static inline int drpc_append_select_encoding(drpc_buffer_t *b, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_SELECT_ENCODING;
  _drpc_store32(buf + 1, size);

  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

static inline int drpc_append_goaway(drpc_buffer_t *b, uint8_t code, uint32_t size, const char* data) {
  #define SIZE sizeof(uint8_t) + sizeof(uint8_t) + sizeof(uint32_t)
  unsigned char buf[SIZE];
  buf[0] = DRPC_OP_GOAWAY;
  buf[1] = code;
  _drpc_store32(buf + 2, size);

  int ret = drpc_buffer_write(b, (const char*) buf, SIZE);
  #undef SIZE

  if (ret < 0)
    return ret;

  drpc_append(b, data, size);
}

#undef drpc_append

#endif