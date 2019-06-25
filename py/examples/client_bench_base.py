from __future__ import absolute_import
from __future__ import print_function
import time
from gevent import GreenletExit
from traceback import print_exc
from six.moves import range

i = 0
max_age = 0.0
request_time = 0.0
in_flight = 0
failed_requests = 0


def run_client_bench(client, concurrency):
    def do_work():
        global i, last, failed_requests, max_age, request_time, in_flight
        start_time = time.time()

        try:
            in_flight += 1
            res = client.send_request('hello world')
            # gevent.sleep(0)
            # print repr(res)
            assert res == 'hello world'
            i += 1
        except GreenletExit:
            raise

        except:
            failed_requests += 1
            print_exc()

        finally:
            age = time.time() - start_time
            request_time += age
            if age > max_age:
                max_age = age

            in_flight -= 1

    def log_loop():
        global max_age, request_time
        last_i = 0
        last = time.time()
        while True:
            gevent.sleep(1)
            now = time.time()
            elapsed = now - last
            req_sec = (i - last_i) / elapsed
            if i - last_i:
                avg_time = request_time / (i - last_i)
            else:
                avg_time = 0

            # max_age = max(now - r.start_time for r in in_flight) if in_flight else 0
            print('%s total requests (%.2f/sec). last log %.2f sec ago. %s failed, %s in flight, %.2f ms max, %.2f ms avg response time' % (
                i, req_sec, elapsed, failed_requests, in_flight, max_age * 1000, avg_time * 1000
            ))
            last_i = i
            last = now
            max_age = 0
            request_time = 0

    def work_loop():
        print('spawning work loop')
        global i, last

        while True:
            do_work()

    import gevent
    client.send_request('hi')
    greenlets = [gevent.spawn(log_loop)]
    for _ in range(concurrency):
        greenlets.append(gevent.spawn(work_loop))

    import signal

    signal.signal(signal.SIGINT, lambda *_: gevent.killall(greenlets, block=False))

    gevent.joinall(greenlets)
    print('All greenlets have exited.')
