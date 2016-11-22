import gevent
from gevent.monkey import patch_all
patch_all()

from drpc.server import DRPCServer


def _gevent_echo(request, session):
    session.send_response(request.seq, request.data)


class Server(DRPCServer):
    def handle_request(self, request, session):
        gevent.spawn_raw(_gevent_echo, request, session)

    def handle_push(self, push, session):
        # print 'psuh'
        return

if __name__ == '__main__':
    s = Server(('localhost', 4001))
    s.serve_forever()
