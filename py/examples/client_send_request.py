from __future__ import absolute_import
from __future__ import print_function
from loqui.client import LoquiClient

client = LoquiClient(('localhost', 4001))
print(len(client.send_request('hello world')))