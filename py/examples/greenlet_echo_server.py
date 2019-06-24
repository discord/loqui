from __future__ import absolute_import
import gevent
from gevent.monkey import patch_all
patch_all()

from loqui.server import LoquiServer


def _gevent_echo(request, session):
    session.send_response(request.seq, request.data)


class Server(LoquiServer):
    def handle_request(self, request, session):
        gevent.spawn_raw(_gevent_echo, request, session)

    def handle_push(self, push, session):
        # print 'psuh'
        return

if __name__ == '__main__':
    s = Server(('localhost', 8080))
    s.serve_forever()
