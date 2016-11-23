#ifndef DRPC_CONSTANTS_H__
#define DRPC_CONSTANTS_H__

#include "sysdep.h"

const unsigned char DRPC_VERSION = 1;
const size_t DRPC_DATA_SIZE_MAX = 4294967295;

typedef enum {
  DRPC_OP_HELLO = 1,
  DRPC_OP_SELECT_ENCODING = 2,
  DRPC_OP_PING = 3,
  DRPC_OP_PONG = 4,
  DRPC_OP_REQUEST = 5,
  DRPC_OP_RESPONSE = 6,
  DRPC_OP_PUSH = 7,
  DRPC_OP_GOAWAY = 8,
  DRPC_OP_ERROR = 9
} drpc_opcodes;


typedef enum {
  DRPC_DECODE_NEEDS_MORE = 1,
  DRPC_DECODE_COMPLETE = 2,
  DRPC_DECODE_MEMORY_ERROR = -1,
  DRPC_DECODE_INVALID_OPCODE = -2,
  DRPC_DECODE_INVALID_SIZE = -3
} drpc_decoder_status;

#endif