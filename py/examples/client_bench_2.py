from client_bench_base import run_client_bench
from gevent.monkey import patch_all
patch_all()

from drpc.client import DRPCClient

client = DRPCClient(('localhost', 4001))
run_client_bench(client, concurrency=100)
