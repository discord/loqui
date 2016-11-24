import struct

import itertools

from loqui.stream_handler import LoquiStreamHandler
from loqui.opcodes import Request


def py_encode_request(seq, data):
    header = struct.pack('>BII', 5, seq, len(data))
    return header + data


def chunker(chunk_size, byt):
    biter = iter(byt)
    return lambda: b''.join(itertools.islice(biter, 0, chunk_size))


def test_stream_handler_encode_request():
    handler = LoquiStreamHandler()
    data = b'hello world'

    assert handler.current_seq() == 0
    seq = handler.send_request(data)
    assert handler.current_seq() == seq
    assert seq == 1

    out_bytes = handler.write_buffer_get_bytes(handler.write_buffer_len())
    header = struct.pack('>BII', 5, seq, len(data))

    assert out_bytes == header + data
    assert handler.write_buffer_len() == 0


def test_stream_handler_encode_requests():
    for chunk_size in xrange(1, 500):

        handler = LoquiStreamHandler()
        expected_data = []
        for i in xrange(1024):
            data = b'hello world - %i' % i

            assert handler.current_seq() == i
            seq = handler.send_request(data)
            assert handler.current_seq() == seq
            assert seq == i + 1

            expected_data.append(struct.pack('>BII', 5, seq, len(data)))
            expected_data.append(data)

        read_data = []

        while handler.write_buffer_len():
            read_data.append(handler.write_buffer_get_bytes(min(handler.write_buffer_len(), chunk_size)))

        assert b''.join(read_data) == b''.join(expected_data)
        assert handler.write_buffer_len() == 0


def test_stream_handler_encode_requests_interspersed():
    for chunk_size in xrange(1, 500):

        handler = LoquiStreamHandler()
        expected_data = []
        read_data = []

        for i in xrange(1024):
            data = b'hello world - %i' % i

            assert handler.current_seq() == i
            seq = handler.send_request(data)
            assert handler.current_seq() == seq
            assert seq == i + 1

            expected_data.append(struct.pack('>BII', 5, seq, len(data)))
            expected_data.append(data)
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
    payload = py_encode_request(1, b'this is a test')
    handler = LoquiStreamHandler()

    responses = handler.on_bytes_received(payload)
    assert len(responses) == 1
    response = responses[0]
    assert isinstance(response, Request)
    assert response.seq == 1
    assert response.data == b'this is a test'


def test_stream_handler_decode_byte_by_byte():
    payload = py_encode_request(1, b'this is a test') + py_encode_request(2, b'this is a test 2')

    for i in xrange(1, len(payload)):

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
        assert response.data == b'this is a test'
        response = responses[1]
        assert isinstance(response, Request)
        assert response.seq == 2
        assert response.data == b'this is a test 2'
