ENCODERS = {}

try:
    import msgpack

    ENCODERS['msgpack'] = msgpack

except ImportError:
    pass

try:
    import json

    ENCODERS['json'] = json

except ImportError:
    pass
