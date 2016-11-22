from drpc.client import DRPCClient

client = DRPCClient(('localhost', 4001))
print client.send_request('hello world').data