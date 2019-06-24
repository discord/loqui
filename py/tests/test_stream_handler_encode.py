from __future__ import absolute_import
from . import harness
from loqui.stream_handler import LoquiStreamHandler


def _test_encoder(encode_fn, expected_result):
    handler = LoquiStreamHandler()
    encode_fn(handler)

    write_buffer = handler.write_buffer_get_bytes(handler.write_buffer_len())
    assert write_buffer == expected_result


def test_encode_op_hello():
    _test_encoder(
        lambda h: h.send_hello(15, [b"msgpack", b"json"], [b"gzip"]),
        harness.encode_hello(15, 1, b"msgpack,json|gzip")
    )


def test_encode_op_hello_ack():
    _test_encoder(
        lambda h: h.send_hello_ack(15, 32000, b"msgpack", b"gzip"),
        harness.encode_hello_ack(15, 32000, b"msgpack|gzip")
    )


def test_encode_op_ping():
    _test_encoder(
        lambda h: h.send_ping(15),
        harness.encode_ping(15, 1)
    )


def test_encode_op_pong():
    _test_encoder(
        lambda h: h.send_pong(15, 300),
        harness.encode_pong(15, 300)
    )


def test_encode_request():
    _test_encoder(
        lambda h: h.send_request(31, b"hello this is my data"),
        harness.encode_request(31, 1, b"hello this is my data")
    )


def test_encode_response():
    _test_encoder(
        lambda h: h.send_response(31, 3000, b"hello this is my data"),
        harness.encode_response(31, 3000, b"hello this is my data")
    )


def test_encode_push():
    _test_encoder(
        lambda h: h.send_push(91, b"hello this is my push"),
        harness.encode_push(91, b"hello this is my push")
    )


def test_encode_go_away():
    _test_encoder(
        lambda h: h.send_goaway(151, 9001, b"go away pls"),
        harness.encode_go_away(151, 9001, b"go away pls")
    )


def test_encode_error():
    _test_encoder(
        lambda h: h.send_error(151, 1444, 900100, b"errrror!"),
        harness.encode_error(151, 1444, 900100, b"errrror!")
    )
