class DRPCException(Exception):
    pass


class NoEncoderAvailable(DRPCException):
    pass


class ConnectionError(DRPCException):
    pass


class ConnectionTerminated(ConnectionError):
    pass


class ConnectionPingTimeout(ConnectionError):
    pass
