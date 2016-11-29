class LoquiException(Exception):
    pass


class LoquiDecoderError(LoquiException):
    pass


class NoEncoderAvailable(LoquiException):
    pass


class ConnectionError(LoquiException):
    pass


class StreamDefunct(ConnectionError):
    pass


class ConnectionTerminated(ConnectionError):
    pass


class ConnectionPingTimeout(ConnectionError):
    pass


class InvalidSendException(LoquiException):
    pass


class NotClientException(InvalidSendException):
    pass


class NotServerException(InvalidSendException):
    pass


class LoquiErrorReceived(LoquiException):
    pass