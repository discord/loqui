from loqui.client import LoquiClient

client = LoquiClient(('localhost', 4001))
print len(client.send_request('hello world'))