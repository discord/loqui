from loqui.client import LoquiClient

client = LoquiClient(('localhost', 4001))
for i in xrange(100):
    client.send_push('hello world %i' % i)

assert client.send_request('oh hi') == 100

for i in xrange(100):
    client.send_push('hello world %i' % i)

assert client.send_request('oh hi') == 200
