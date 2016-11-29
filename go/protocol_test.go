package loqui

import (
	"bytes"
	"reflect"
	"testing"
)

var expectedFlags uint8 = 1 << 5

func TestProtocolHello(t *testing.T) {
	expectedVersion := uint8(1)
	expectedEncodings := []string{"a", "b"}
	expectedCompressions := []string{"c", "d"}
	testProtocolOp(t, opHello,
		func(pw *protocolWriter) error {
			return pw.writeHello(expectedFlags, expectedVersion, expectedEncodings, expectedCompressions)
		},
		func(pr *protocolReader) error {
			version, encodings, compressions, err := pr.readHello()
			if version != expectedVersion {
				t.Fatalf("unexpected version: %d. Expecting %d", version, expectedVersion)
			}
			if !reflect.DeepEqual(encodings, expectedEncodings) {
				t.Fatalf("unexpected encodings: %v. Expecting %v", encodings, expectedEncodings)
			}
			if !reflect.DeepEqual(compressions, expectedCompressions) {
				t.Fatalf("unexpected compressions: %v. Expecting %v", compressions, expectedCompressions)
			}
			return err
		},
	)
}

func TestProtocolHelloAck(t *testing.T) {
	var expectedPingInterval uint32 = 30000
	expectedEncoding := "a"
	expectedCompression := "c"
	testProtocolOp(t, opHelloAck,
		func(pw *protocolWriter) error {
			return pw.writeHelloAck(expectedFlags, expectedPingInterval, expectedEncoding, expectedCompression)
		},
		func(pr *protocolReader) error {
			pingInterval, encoding, compression, err := pr.readHelloAck()
			if pingInterval != expectedPingInterval {
				t.Fatalf("unexpected pingInterval: %d. Expecting %d", pingInterval, expectedPingInterval)
			}
			if encoding != expectedEncoding {
				t.Fatalf("unexpected encoding: %s. Expecting %s", encoding, expectedEncoding)
			}
			if compression != expectedCompression {
				t.Fatalf("unexpected compression: %s. Expecting %s", compression, expectedCompression)
			}
			return err
		},
	)
}

func TestProtocolPing(t *testing.T) {
	var expectedSeq uint32 = 111
	testProtocolOp(t, opPing,
		func(pw *protocolWriter) error {
			return pw.writePing(expectedFlags, expectedSeq)
		},
		func(pr *protocolReader) error {
			seq, err := pr.readPing()
			if seq != expectedSeq {
				t.Fatalf("unexpected seq: %d. Expecting %d", seq, expectedSeq)
			}
			return err
		},
	)
}

func TestProtocolPong(t *testing.T) {
	var expectedSeq uint32 = 111
	testProtocolOp(t, opPong,
		func(pw *protocolWriter) error {
			return pw.writePong(expectedFlags, expectedSeq)
		},
		func(pr *protocolReader) error {
			seq, err := pr.readPong()
			if seq != expectedSeq {
				t.Fatalf("unexpected seq: %d. Expecting %d", seq, expectedSeq)
			}
			return err
		},
	)
}

func TestProtocolRequest(t *testing.T) {
	var expectedSeq uint32 = 111
	expectedPayload := []byte("hello world")
	testProtocolOp(t, opRequest,
		func(pw *protocolWriter) error {
			return pw.writeRequest(expectedFlags, expectedSeq, expectedPayload)
		},
		func(pr *protocolReader) error {
			seq, payload, err := pr.readRequest()
			if seq != expectedSeq {
				t.Fatalf("unexpected seq: %d. Expecting %d", seq, expectedSeq)
			}
			if !bytes.Equal(payload.Bytes(), expectedPayload) {
				t.Fatalf("unexpected payload: %v. Expecting %v", payload.Bytes(), expectedPayload)
			}
			return err
		},
	)
}

func TestProtocolResponse(t *testing.T) {
	var expectedSeq uint32 = 111
	expectedPayload := []byte("hello world")
	testProtocolOp(t, opResponse,
		func(pw *protocolWriter) error {
			return pw.writeResponse(expectedFlags, expectedSeq, expectedPayload)
		},
		func(pr *protocolReader) error {
			seq, payload, err := pr.readResponse()
			if seq != expectedSeq {
				t.Fatalf("unexpected seq: %d. Expecting %d", seq, expectedSeq)
			}
			if !bytes.Equal(payload.Bytes(), expectedPayload) {
				t.Fatalf("unexpected payload: %v. Expecting %v", payload.Bytes(), expectedPayload)
			}
			return err
		},
	)
}

func TestProtocolPush(t *testing.T) {
	expectedPayload := []byte("hello world")
	testProtocolOp(t, opPush,
		func(pw *protocolWriter) error {
			return pw.writePush(expectedFlags, expectedPayload)
		},
		func(pr *protocolReader) error {
			payload, err := pr.readPush()
			if !bytes.Equal(payload.Bytes(), expectedPayload) {
				t.Fatalf("unexpected payload: %v. Expecting %v", payload.Bytes(), expectedPayload)
			}
			return err
		},
	)
}

func TestProtocolError(t *testing.T) {
	var expectedSeq uint32 = 111
	var expectedCode uint16 = 1
	expectedReason := "testing"
	testProtocolOp(t, opError,
		func(pw *protocolWriter) error {
			return pw.writeError(expectedFlags, expectedSeq, expectedCode, expectedReason)
		},
		func(pr *protocolReader) error {
			seq, code, reason, err := pr.readError()
			if seq != expectedSeq {
				t.Fatalf("unexpected seq: %d. Expecting %d", seq, expectedSeq)
			}
			if code != expectedCode {
				t.Fatalf("unexpected code: %d. Expecting %d", code, expectedCode)
			}
			if reason != expectedReason {
				t.Fatalf("unexpected reason: %s. Expecting %s", reason, expectedReason)
			}
			return err
		},
	)
}

func TestGoAway(t *testing.T) {
	var expectedCode uint16 = 1
	expectedReason := "testing"
	testProtocolOp(t, opGoAway,
		func(pw *protocolWriter) error {
			return pw.writeGoAway(expectedFlags, expectedCode, expectedReason)
		},
		func(pr *protocolReader) error {
			code, reason, err := pr.readGoAway()
			if code != expectedCode {
				t.Fatalf("unexpected code: %d. Expecting %d", code, expectedCode)
			}
			if reason != expectedReason {
				t.Fatalf("unexpected reason: %s. Expecting %s", reason, expectedReason)
			}
			return err
		},
	)
}

func testProtocolOp(t *testing.T, expectedOp uint8, write func(pw *protocolWriter) error, read func(pr *protocolReader) error) {
	b := new(bytes.Buffer)

	pw := newProtocolWriter(b)
	pr := newProtocolReader(b, 4096)

	err := write(&pw)
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}

	op, err := pr.readOp()
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}

	if op != expectedOp {
		t.Fatalf("unexpected op: %d. Expecting %d", op, expectedOp)
	}

	flags, err := pr.readFlags()
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}

	if flags != expectedFlags {
		t.Fatalf("unexpected flags: %d. Expecting %d", flags, expectedFlags)
	}

	err = read(&pr)
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}
}
