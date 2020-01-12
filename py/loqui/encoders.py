from __future__ import absolute_import
from collections import OrderedDict

ENCODERS = OrderedDict()

try:
    import msgpack

    ENCODERS[b'msgpack'] = msgpack

except ImportError:
    pass

try:
    import json

    ENCODERS[b'json'] = json

except ImportError:
    pass

try:
    import erlpack

    # Make erlpack conform to dumps/loads.
    class _erlpack:
        dumps = erlpack.pack
        loads = erlpack.unpack

    ENCODERS[b'erlpack'] = _erlpack

except ImportError:
    pass
