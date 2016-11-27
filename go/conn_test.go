package loqui

import (
	"bytes"
	"io"
	"io/ioutil"
	"net"
	"testing"
	"time"
)

func TestSelectEncoding(t *testing.T) {
	client, server := newPair()
	defer server.Close(0)

	encoding, err := client.Encoding()
	expectedEncoding := "msgpack"
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}
	if encoding != expectedEncoding {
		t.Fatalf("unexpected encoding: %s. Expecting %s", encoding, expectedEncoding)
	}
}

func TestRequest(t *testing.T) {
	client, _ := newPair()

	expectedPayload := []byte("hello world")
	b, err := client.RequestTimeout(expectedPayload, false, time.Second)
	if err != nil {
		t.Fatalf("unexpected error: %s", err)
	}

	payload, _ := ioutil.ReadAll(b)
	if !bytes.Equal(payload, expectedPayload) {
		t.Fatalf("unexpected payload: %s. Expecting %s", payload, expectedPayload)
	}
}

type serverHandler []byte

func (s serverHandler) ServeRequest(ctx RequestContext) {
	io.CopyBuffer(ctx, ctx, s)
}

func newPair() (*Conn, *Conn) {
	a, b := net.Pipe()

	client := NewConn(a, a, a, true)
	server := NewConn(b, b, b, false)
	server.handler = make(serverHandler, 1024)

	encodings := []string{"msgpack", "json"}
	client.supportedEncodings = encodings
	server.supportedEncodings = encodings
	server.pingInterval = time.Second * 5

	go server.Serve(100)
	client.AwaitReady(time.Second)

	return client, server
}
