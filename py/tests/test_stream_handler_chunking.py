from __future__ import absolute_import
import itertools
from . import harness

from loqui.stream_handler import LoquiStreamHandler
from loqui.opcodes import Request

import six
from six.moves import range


def chunker(chunk_size, byt):
    biter = iter(byt)
    return lambda: six.ensure_binary(''.join([chr(x) if isinstance(x, int) else x for x in itertools.islice(biter, 0, chunk_size)]))


def test_stream_handler_encode_requests():
    for chunk_size in range(1, 500):

        handler = LoquiStreamHandler()
        expected_data = []
        for i in range(1024):
            data = b'hello world - %i' % i

            assert handler.current_seq() == i
            seq = handler.send_request(1, data)
            assert handler.current_seq() == seq
            assert seq == i + 1

            expected_data.append(harness.encode_request(1, seq, data))

        read_data = []

        while handler.write_buffer_len():
            read_data.append(handler.write_buffer_get_bytes(min(handler.write_buffer_len(), chunk_size)))

        assert b''.join(read_data) == b''.join(expected_data)
        assert handler.write_buffer_len() == 0


def test_stream_handler_encode_requests_interspersed():
    for chunk_size in range(1, 500):

        handler = LoquiStreamHandler()
        expected_data = []
        read_data = []

        for i in range(1024):
            data = b'hello world - %i' % i

            assert handler.current_seq() == i
            seq = handler.send_request(5, data)
            assert handler.current_seq() == seq
            assert seq == i + 1

            expected_data.append(harness.encode_request(5, seq, data))

            if handler.write_buffer_len():
                read_data.append(handler.write_buffer_get_bytes(min(handler.write_buffer_len(), chunk_size)))

        while handler.write_buffer_len():
            read_data.append(handler.write_buffer_get_bytes(min(handler.write_buffer_len(), chunk_size)))

        assert b''.join(read_data) == b''.join(expected_data)
        assert handler.write_buffer_len() == 0


def test_stream_handler_write_buffer_overread_is_ok():
    handler = LoquiStreamHandler()
    assert handler.write_buffer_get_bytes(1000) is None
    assert handler.write_buffer_get_bytes(0) is None


def test_stream_handler_decode_full():
    payload = harness.encode_request(37, 1, b'this is a test')
    handler = LoquiStreamHandler()

    responses = handler.on_bytes_received(payload)
    assert len(responses) == 1
    response = responses[0]
    assert isinstance(response, Request)
    assert response.seq == 1
    assert response.flags == 37
    assert response.data == b'this is a test'


def test_stream_handler_decode_byte_by_byte():
    payload = harness.encode_request(0, 1, b'this is a test') + harness.encode_request(5, 2, b'this is a test 2')

    for i in range(1, len(payload)):
        handler = LoquiStreamHandler()
        chunk_get = chunker(i, payload)
        responses = []
        while True:
            chunk = chunk_get()
            if not chunk:
                break

            responses += handler.on_bytes_received(chunk)

        assert len(responses) == 2
        response = responses[0]
        assert isinstance(response, Request)
        assert response.seq == 1
        assert response.flags == 0
        assert response.data == b'this is a test'
        response = responses[1]
        assert isinstance(response, Request)
        assert response.seq == 2
        assert response.flags == 5
        assert response.data == b'this is a test 2'
