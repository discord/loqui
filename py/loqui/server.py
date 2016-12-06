import socket
from gevent.server import StreamServer

from loqui.encoders import ENCODERS
from loqui.socket_session import LoquiSocketSession


class LoquiServer:
    def __init__(self, server_address):
        self.server = StreamServer(server_address, self._handle_connection)

    def serve_forever(self):
        self.server.serve_forever()

    def start(self):
        self.server.start()

    def stop(self):
        self.server.stop()

    def _handle_connection(self, sock, addr):
        print 'handling connection from', sock
        sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        sock.setblocking(False)
        session = LoquiSocketSession(sock, ENCODERS, False, self.handle_request, self.handle_push)

        self.handle_new_session(session)
        try:
            session.join()
        finally:
            self.handle_session_gone(session)

    def handle_request(self, request, session):
        pass

    def handle_push(self, push, session):
        pass

    def handle_new_session(self, session):
        pass

    def handle_session_gone(self, session):
        pass
