from gevent.monkey import patch_all
patch_all()

from client_bench_base import run_client_bench
from loqui.client import LoquiHTTPUpgradeCient
client = LoquiHTTPUpgradeCient(('localhost', 8080))
run_client_bench(client, concurrency=100)
