from __future__ import absolute_import
import struct
import six

uint8_t = lambda v: ('B', v)
uint16_t = lambda v: ('H', v)
uint32_t = lambda v: ('I', v)
payload = lambda v: ('payload', v)


class Opcodes:
    HELLO = uint8_t(1)
    HELLO_ACK = uint8_t(2)
    PING = uint8_t(3)
    PONG = uint8_t(4)
    REQUEST = uint8_t(5)
    RESPONSE = uint8_t(6)
    PUSH = uint8_t(7)
    GO_AWAY = uint8_t(8)
    ERROR = uint8_t(9)


def pack(*struct_types):
    fmt = ['>']
    values = []
    extra = []
    for fmt_char, value in struct_types:
        if fmt_char == 'payload':
            extra.append(value)
            fmt_char = 'I'
            value = len(value)

        fmt.append(fmt_char)
        values.append(value)

    val = struct.pack(six.ensure_binary(''.join(fmt)), *values)
    if extra:
        val += b''.join(extra)

    return val


def encode_hello(flags, version, data):
    return pack(
        Opcodes.HELLO,
        uint8_t(flags),
        uint8_t(version),
        payload(data)
    )


def encode_hello_ack(flags, ping_interval, data):
    return pack(
        Opcodes.HELLO_ACK,
        uint8_t(flags),
        uint32_t(ping_interval),
        payload(data)
    )


def encode_ping(flags, seq):
    return pack(
        Opcodes.PING,
        uint8_t(flags),
        uint32_t(seq)
    )


def encode_pong(flags, seq):
    return pack(
        Opcodes.PONG,
        uint8_t(flags),
        uint32_t(seq)
    )


def encode_request(flags, seq, data):
    return pack(
        Opcodes.REQUEST,
        uint8_t(flags),
        uint32_t(seq),
        payload(data)
    )


def encode_response(flags, seq, data):
    return pack(
        Opcodes.RESPONSE,
        uint8_t(flags),
        uint32_t(seq),
        payload(data)
    )


def encode_push(flags, data):
    return pack(
        Opcodes.PUSH,
        uint8_t(flags),
        payload(data)
    )


def encode_go_away(flags, close_code, data):
    return pack(
        Opcodes.GO_AWAY,
        uint8_t(flags),
        uint16_t(close_code),
        payload(data)
    )


def encode_error(flags, code, seq, data):
    return pack(
        Opcodes.ERROR,
        uint8_t(flags),
        uint32_t(seq),
        uint16_t(code),
        payload(data)
    )
