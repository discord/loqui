class DRPCException(Exception):
    pass


class DRPCDecoderError(DRPCException):
    pass


class NoEncoderAvailable(DRPCException):
    pass


class ConnectionError(DRPCException):
    pass


class StreamDefunct(ConnectionError):
    pass


class ConnectionTerminated(ConnectionError):
    pass


class ConnectionPingTimeout(ConnectionError):
    pass


class InvalidSendException(DRPCException):
    pass


class NotClientException(InvalidSendException):
    pass


class NotServerException(InvalidSendException):
    pass
