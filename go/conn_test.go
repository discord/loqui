package loqui

import (
	"net"
	"testing"
)

func TestSelectEncoding(t *testing.T) {
	client, server := newPair()
	defer server.Close(0)
	go server.Serve(1)

	encoding, err := client.Encoding()
	expectedEncoding := "msgpack"
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}
	if encoding != expectedEncoding {
		t.Fatalf("unexpected encoding: %s. Expecting %s", encoding, expectedEncoding)
	}
}

func newPair() (*Conn, *Conn) {
	a, b := net.Pipe()

	client := NewConn(a, a, a, true)
	server := NewConn(b, b, b, false)

	encodings := []string{"msgpack", "json"}
	client.supportedEncodings = encodings
	server.supportedEncodings = encodings

	return client, server
}
