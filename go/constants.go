package loqui

const (
	version uint8 = 1
)

const (
	opHello uint8 = iota + 1
	opSelectEncoding
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
	// CodePingTimeout is sent when connection does not receive a pong within ping interval.
	CodePingTimeout
	// CodeInvalidEncoding is sent when there are no matching encodings.
	CodeInvalidEncoding
)
