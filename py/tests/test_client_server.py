import pytest

from loqui.client import LoquiClient
from loqui.server import LoquiServer


@pytest.fixture()
def request_data():
    return b'request'


@pytest.fixture()
def response():
    return b'response'


@pytest.fixture()
def server(response, request_data):
    class TestServer(LoquiServer):
        def handle_request(self, request, session):
            if request.data != request_data:
                return b'unexpected request data %s' % (request.data,)
            return response
    s = TestServer(('localhost', 0))
    s.start()
    yield s
    s.stop()


@pytest.fixture()
def client(server):
    port = server.server.socket.getsockname()[1]
    c = LoquiClient(('localhost', port))
    return c


def test_smoke(server, client, response, request_data):
    assert client.send_request(request_data, timeout=1) == response

