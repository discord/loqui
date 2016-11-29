package loqui

const (
	version uint8 = 1
)

const (
	opHello uint8 = iota + 1
	opHelloAck
	opPing
	opPong
	opRequest
	opResponse
	opPush
	opGoAway
	opError
)

const (
	// CodeNormal is sent when the connection is closing cleanly.
	CodeNormal uint16 = iota
	// CodeInvalidOp is sent when the connection receives an opcode it cannot handle.
	CodeInvalidOp
	// CodeUnsupportedVersion is sent when conn does not support a version.
	CodeUnsupportedVersion
	// CodeNoCommonEncoding is sent when there are no common encodings.
	CodeNoCommonEncoding
	// CodeInvalidEncoding is sent by the client if the server chooses and invalid encoding.
	CodeInvalidEncoding
	// CodeInvalidCompression is sent by the client if the server chooses and invalid compression.
	CodeInvalidCompression
	// CodePingTimeout is sent when connection does not receive a pong within ping interval.
	CodePingTimeout
	// CodeInternalServerError is sent when a single request dies due to an error.
	CodeInternalServerError
)

const (
	// FlagNone is sent when frame has no flags.
	FlagNone uint8 = 0
	// FlagCompressed is sent when the payload is compressed
	FlagCompressed = 1 << 0
)
