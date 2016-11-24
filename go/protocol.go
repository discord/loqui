package loqui

import (
	"bufio"
	"bytes"
	"encoding/binary"
	"io"
	"strings"
)

type protocolHandler interface {
	handleHello(version uint8, pingInterval uint32, encodings []string)
	handleSelectEncoding(encoding string)
	handlePing(seq uint32)
	handlePong(seq uint32)
	handleRequest(seq uint32, payload *bytes.Buffer)
	handleResponse(seq uint32, payload *bytes.Buffer)
	handlePush(payload *bytes.Buffer)
	handleError(seq uint32, code uint16, reason string)
	handleGoAway(code uint16, reason string)
}

type protocolReader struct {
	buf *bytes.Buffer
	br  *bufio.Reader
}

func newProtocolReader(r io.Reader) protocolReader {
	var br *bufio.Reader
	var ok bool
	if br, ok = r.(*bufio.Reader); !ok {
		br = bufio.NewReader(r)
	}
	return protocolReader{
		buf: new(bytes.Buffer),
		br:  br,
	}
}

func (pr *protocolReader) read(n int) (v []byte, err error) {
	pr.buf.Reset()
	pr.buf.Grow(n)
	v = pr.buf.Bytes()[:n]
	_, err = pr.br.Read(v)
	return
}

func (pr *protocolReader) readInt() (int, error) {
	v, err := pr.readUint32()
	return int(v), err
}

func (pr *protocolReader) readUint8() (uint8, error) {
	v, err := pr.read(1)
	if err != nil {
		return 0, err
	}
	return v[0], nil
}

func (pr *protocolReader) readUint16() (uint16, error) {
	v, err := pr.read(2)
	if err != nil {
		return 0, err
	}
	return binary.BigEndian.Uint16(v), nil
}

func (pr *protocolReader) readUint32() (uint32, error) {
	v, err := pr.read(4)
	if err != nil {
		return 0, err
	}
	return binary.BigEndian.Uint32(v), nil
}

func (pr *protocolReader) readPayload() (payload *bytes.Buffer, err error) {
	var len int
	var v []byte
	len, err = pr.readInt()
	if err != nil {
		return
	}
	payload = acquireBuffer(len)
	v, err = pr.read(len)
	if err != nil {
		releaseBuffer(payload)
		payload = nil
		return
	}
	payload.Write(v)
	return
}

func (pr *protocolReader) readOp() (uint8, error) {
	return pr.readUint8()
}

func (pr *protocolReader) readHello() (version uint8, pingInterval uint32, encodings []string, err error) {
	version, err = pr.readUint8()
	pingInterval, err = pr.readUint32()
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		encodings = strings.Split(string(payload.Bytes()), ",")
	}
	releaseBuffer(payload)
	return
}

func (pr *protocolReader) readSelectEncoding() (encoding string, err error) {
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		encoding = string(payload.Bytes())
	}
	releaseBuffer(payload)
	return
}

func (pr *protocolReader) readPing() (uint32, error) {
	return pr.readUint32()
}

func (pr *protocolReader) readPong() (uint32, error) {
	return pr.readUint32()
}

func (pr *protocolReader) readRequest() (seq uint32, payload *bytes.Buffer, err error) {
	seq, err = pr.readUint32()
	payload, err = pr.readPayload()
	return
}

func (pr *protocolReader) readResponse() (seq uint32, payload *bytes.Buffer, err error) {
	seq, err = pr.readUint32()
	payload, err = pr.readPayload()
	return
}

func (pr *protocolReader) readPush() (*bytes.Buffer, error) {
	return pr.readPayload()
}

func (pr *protocolReader) readError() (seq uint32, code uint16, reason string, err error) {
	seq, err = pr.readUint32()
	code, err = pr.readUint16()
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		reason = string(payload.Bytes())
	}
	releaseBuffer(payload)
	return
}

func (pr *protocolReader) readGoAway() (code uint16, reason string, err error) {
	code, err = pr.readUint16()
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		reason = string(payload.Bytes())
	}
	releaseBuffer(payload)
	return
}

func (pr *protocolReader) process(ph protocolHandler) error {
	op, err := pr.readOp()
	if err != nil {
		return err
	}
	switch op {
	case opHello:
		version, pingInterval, encodings, err := pr.readHello()
		if err != nil {
			return err
		}
		ph.handleHello(version, pingInterval, encodings)
	case opSelectEncoding:
		encoding, err := pr.readSelectEncoding()
		if err != nil {
			return err
		}
		ph.handleSelectEncoding(encoding)
	case opPing:
		seq, err := pr.readPing()
		if err != nil {
			return err
		}
		ph.handlePing(seq)
	case opPong:
		seq, err := pr.readPong()
		if err != nil {
			return err
		}
		ph.handlePong(seq)
	case opRequest:
		seq, payload, err := pr.readRequest()
		if err != nil {
			return err
		}
		ph.handleRequest(seq, payload)
	case opResponse:
		seq, payload, err := pr.readResponse()
		if err != nil {
			return err
		}
		ph.handleResponse(seq, payload)
		releaseBuffer(payload)
	case opPush:
		payload, err := pr.readPush()
		if err != nil {
			return err
		}
		ph.handlePush(payload)
	case opError:
		seq, code, reason, err := pr.readError()
		if err != nil {
			return err
		}
		ph.handleError(seq, code, reason)
	case opGoAway:
		code, reason, err := pr.readGoAway()
		if err != nil {
			return err
		}
		ph.handleGoAway(code, reason)
	}
	return nil
}

type protocolWriter struct {
	w io.Writer
}

func newProtocolWriter(w io.Writer) protocolWriter {
	return protocolWriter{w: w}
}

func (pw *protocolWriter) Write(p []byte) (int, error) {
	return pw.w.Write(p)
}

func (pw *protocolWriter) writeHello(pingInterval uint32, encodings []string) error {
	const headerLen = 10
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = opHello
	header[1:2][0] = version
	binary.BigEndian.PutUint32(header[2:6], pingInterval)
	encodingsString := strings.Join(encodings, ",")
	binary.BigEndian.PutUint32(header[6:headerLen], uint32(len(encodingsString)))
	_, err := pw.Write(append(header[:headerLen], encodingsString...))
	releaseBuffer(buf)
	return err
}

func (pw *protocolWriter) writeSelectEncoding(encoding string) error {
	const headerLen = 5
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = opSelectEncoding
	binary.BigEndian.PutUint32(header[1:headerLen], uint32(len(encoding)))
	_, err := pw.Write(append(header[:headerLen], encoding...))
	releaseBuffer(buf)
	return err
}

func (pw *protocolWriter) writePingPong(op uint8, seq uint32) error {
	const headerLen = 5
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = op
	binary.BigEndian.PutUint32(header[1:headerLen], seq)
	_, err := pw.Write(header[:headerLen])
	releaseBuffer(buf)
	return err
}

func (pw *protocolWriter) writePing(seq uint32) error {
	return pw.writePingPong(opPing, seq)
}

func (pw *protocolWriter) writePong(seq uint32) error {
	return pw.writePingPong(opPong, seq)
}

func (pw *protocolWriter) writeRequestResponse(op uint8, seq uint32, payload []byte) (err error) {
	const headerLen = 9
	buf := acquireBuffer(headerLen + len(payload))
	header := buf.Bytes()
	header[:1][0] = op
	binary.BigEndian.PutUint32(header[1:5], seq)
	binary.BigEndian.PutUint32(header[5:headerLen], uint32(len(payload)))
	_, err = pw.Write(append(header[:headerLen], payload...))
	releaseBuffer(buf)
	return
}

func (pw *protocolWriter) writeRequest(seq uint32, payload []byte) error {
	return pw.writeRequestResponse(opRequest, seq, payload)
}

func (pw *protocolWriter) writeResponse(seq uint32, payload []byte) error {
	return pw.writeRequestResponse(opResponse, seq, payload)
}

func (pw *protocolWriter) writePush(payload []byte) (err error) {
	const headerLen = 5
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = opPush
	binary.BigEndian.PutUint32(header[1:headerLen], uint32(len(payload)))
	_, err = pw.Write(append(header[:headerLen], payload...))
	releaseBuffer(buf)
	return
}

func (pw *protocolWriter) writeError(seq uint32, code uint16, reason string) (err error) {
	const headerLen = 11
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = opError
	binary.BigEndian.PutUint32(header[1:5], seq)
	binary.BigEndian.PutUint16(header[5:7], code)
	binary.BigEndian.PutUint32(header[7:headerLen], uint32(len(reason)))
	_, err = pw.Write(append(header[:headerLen], reason...))
	releaseBuffer(buf)
	return
}

func (pw *protocolWriter) writeGoAway(code uint16, reason string) (err error) {
	const headerLen = 7
	buf := acquireBuffer(headerLen)
	header := buf.Bytes()
	header[:1][0] = opGoAway
	binary.BigEndian.PutUint16(header[1:3], code)
	binary.BigEndian.PutUint32(header[3:headerLen], uint32(len(reason)))
	_, err = pw.Write(append(header[:headerLen], reason...))
	releaseBuffer(buf)
	return
}

type protocol struct {
	protocolReader
	protocolWriter
}

func newProtocol(rw io.ReadWriter) protocol {
	return protocol{
		newProtocolReader(rw),
		newProtocolWriter(rw),
	}
}
