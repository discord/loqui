from __future__ import absolute_import
from . import harness
from loqui import opcodes
from loqui.stream_handler import LoquiStreamHandler


def _test_decoder(buffer):
    handler = LoquiStreamHandler()
    result = handler.on_bytes_received(buffer)
    assert len(result) == 1
    return result[0]


def test_decode_op_hello():
    hello = _test_decoder(
        harness.encode_hello(15, 1, b"msgpack,json|gzip,lzma")
    )

    assert isinstance(hello, opcodes.Hello)
    assert hello.version == 1
    assert hello.flags == 15
    assert hello.supported_encodings == [b'msgpack', b'json']
    assert hello.supported_compressions == [b'gzip', b'lzma']


def test_decode_op_hello_ack():
    hello_ack = _test_decoder(
        harness.encode_hello_ack(15, 32000, b"msgpack|gzip")
    )

    assert isinstance(hello_ack, opcodes.HelloAck)
    assert hello_ack.flags == 15
    assert hello_ack.ping_interval == 32000
    assert hello_ack.selected_encoding == b'msgpack'
    assert hello_ack.selected_compression == b'gzip'


def test_decode_op_ping():
    ping = _test_decoder(
        harness.encode_ping(15, 1)
    )
    assert isinstance(ping, opcodes.Ping)
    assert ping.flags == 15
    assert ping.seq == 1


def test_decode_op_pong():
    pong = _test_decoder(
        harness.encode_pong(15, 300)
    )

    assert isinstance(pong, opcodes.Pong)
    assert pong.flags == 15
    assert pong.seq == 300


def test_decode_request():
    request = _test_decoder(
        harness.encode_request(31, 1, b"hello this is my data")
    )

    assert isinstance(request, opcodes.Request)
    assert request.flags == 31
    assert request.seq == 1
    assert request.data == b"hello this is my data"


def test_decode_response():
    response = _test_decoder(
        harness.encode_response(31, 3000, b"hello this is my data")
    )

    assert isinstance(response, opcodes.Response)
    assert response.flags == 31
    assert response.seq == 3000
    assert response.data == b"hello this is my data"


def test_decode_push():
    push = _test_decoder(
        harness.encode_push(91, b"hello this is my push")
    )

    assert isinstance(push, opcodes.Push)
    assert push.flags == 91
    assert push.data == b"hello this is my push"


def test_decode_go_away():
    go_away = _test_decoder(
        harness.encode_go_away(151, 9001, b"go away pls")
    )

    assert isinstance(go_away, opcodes.GoAway)
    assert go_away.flags == 151
    assert go_away.code == 9001
    assert go_away.reason == b"go away pls"


def test_decode_error():
    error = _test_decoder(
        harness.encode_error(151, 1444, 900100, b"errrror!")
    )

    assert isinstance(error, opcodes.Error)
    assert error.flags == 151
    assert error.code == 1444
    assert error.seq == 900100
    assert error.data == b"errrror!"
