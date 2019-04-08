from client_bench_base import run_client_bench
from gevent.monkey import patch_all
patch_all()

from loqui.client import LoquiClient

client = LoquiClient(('localhost', 8080))
run_client_bench(client, concurrency=100)
