package loqui

import (
	"bytes"
	"reflect"
	"testing"
)

func TestProtocolHello(t *testing.T) {
	var expectedPingInterval uint32 = 30000
	expectedEncodings := []string{"a", "b"}
	testProtocolOp(t, opHello,
		func(pw *protocolWriter) error {
			return pw.writeHello(expectedPingInterval, expectedEncodings)
		},
		func(pr *protocolReader) error {
			_, pingInterval, encodings, err := pr.readHello()
			if pingInterval != expectedPingInterval {
				t.Fatalf("unexpected pingInterval: %d. Expecting %d", pingInterval, expectedPingInterval)
			}
			if !reflect.DeepEqual(encodings, expectedEncodings) {
				t.Fatalf("unexpected encodings: %v. Expecting %v", encodings, expectedEncodings)
			}
			return err
		},
	)
}

func TestProtocolSelectEncoding(t *testing.T) {
	expectedEncoding := "a"
	testProtocolOp(t, opSelectEncoding,
		func(pw *protocolWriter) error {
			return pw.writeSelectEncoding(expectedEncoding)
		},
		func(pr *protocolReader) error {
			encoding, err := pr.readSelectEncoding()
			if encoding != expectedEncoding {
				t.Fatalf("unexpected encoding: %s. Expecting %s", encoding, expectedEncoding)
			}
			return err
		},
	)
}

func TestProtocolPing(t *testing.T) {
	var expectedSeq uint32 = 111
	testProtocolOp(t, opPing,
		func(pw *protocolWriter) error {
			return pw.writePing(expectedSeq)
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
			return pw.writePong(expectedSeq)
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
			return pw.writeRequest(expectedSeq, expectedPayload)
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
			return pw.writeResponse(expectedSeq, expectedPayload)
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
			return pw.writePush(expectedPayload)
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
			return pw.writeError(expectedSeq, expectedCode, expectedReason)
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
			return pw.writeGoAway(expectedCode, expectedReason)
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
	pr := newProtocolReader(b)

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

	err = read(&pr)
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}
}
