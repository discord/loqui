import time
from greenlet import GreenletExit
from traceback import print_exc

from gevent.monkey import patch_all
patch_all()

from drpc.client import DRPCClient


i = 0
in_flight = set()
failed_requests = 0

client = DRPCClient(('localhost', 4001))


class Request(object):
    def __init__(self):
        self.start_time = time.time()


def do_work():
    global i, last, failed_requests
    r = Request()
    in_flight.add(r)

    try:
        res = client.send_request('hello world')
        assert res.data == 'OK!'
        i += 1
    except GreenletExit:
        raise

    except:
        failed_requests += 1
        print_exc()

    finally:
        in_flight.remove(r)


def log_loop():
    last_i = 0
    last = time.time()
    while True:
        gevent.sleep(1)
        now = time.time()
        elapsed = now - last
        req_sec = (i - last_i) / elapsed

        max_age = max(now - r.start_time for r in in_flight) if in_flight else 0
        print '%s total requests (%.2f/sec). last log %.2f sec ago. %s failed, %s in flight, %.2f ms max in flight age' % (
            i, req_sec, elapsed, failed_requests, len(in_flight), max_age * 1000
        )
        last_i = i
        last = now


def work_loop():
    print 'spawning work loop'
    global i, last

    while True:
        do_work()
        # gevent.spawn(do_work)
        # gevent.sleep(random.random() + .5)


import gevent

greenlets = [gevent.spawn(log_loop)]
for _ in xrange(150):
    greenlets.append(gevent.spawn(work_loop))

import signal

signal.signal(signal.SIGINT, lambda *_: gevent.killall(greenlets, block=False))

gevent.joinall(greenlets)
print 'All greenlets have exited.'
