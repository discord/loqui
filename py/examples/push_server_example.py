from __future__ import absolute_import
from loqui.server import LoquiServer

sessions = {}


class Server(LoquiServer):
    def handle_request(self, request, session):
        return sessions[session]

    def handle_push(self, push, session):
        sessions[session] += 1

    def handle_new_session(self, session):
        sessions[session] = 0

    def handle_session_gone(self, session):
        sessions.pop(session, None)


if __name__ == '__main__':
    s = Server(('localhost', 4001))
    s.serve_forever()
