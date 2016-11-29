package loqui

import (
	"bufio"
	"bytes"
	"encoding/binary"
	"errors"
	"io"
	"math"
	"strings"
)

var (
	errUnknownOp = errors.New("loqui: unknown op")
	errNotEnoughSettings = errors.New("loqui: not enough settings")
	errPayloadTooLarge = errors.New("loqui: payload too large")
)

type protocolHandler interface {
	handleHello(flags uint8, version uint8, encodings []string, compressions []string)
	handleHelloAck(flags uint8, pingInterval uint32, encoding string, compression string)
	handlePing(flags uint8, seq uint32)
	handlePong(flags uint8, seq uint32)
	handleRequest(flags uint8, seq uint32, payload *bytes.Buffer)
	handleResponse(flags uint8, seq uint32, payload *bytes.Buffer)
	handlePush(flags uint8, payload *bytes.Buffer)
	handleError(flags uint8, seq uint32, code uint16, reason string)
	handleGoAway(flags uint8, code uint16, reason string)
}

type protocolReader struct {
	buf            *bytes.Buffer
	br             *bufio.Reader
	maxPayloadSize int
}

func newProtocolReader(r io.Reader, maxPayloadSize int) protocolReader {
	var br *bufio.Reader
	var ok bool
	if br, ok = r.(*bufio.Reader); !ok {
		br = bufio.NewReader(r)
	}
	if maxPayloadSize <= 0 {
		maxPayloadSize = math.MaxInt32
	}
	return protocolReader{
		buf:            new(bytes.Buffer),
		br:             br,
		maxPayloadSize: maxPayloadSize,
	}
}

func (pr *protocolReader) read(n int) (v []byte, err error) {
	pr.buf.Reset()
	pr.buf.Grow(n)
	v = pr.buf.Bytes()[:n]
	_, err = io.ReadFull(pr.br, v)
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

func (pr *protocolReader) readPayload() (b *bytes.Buffer, err error) {
	var l int
	var v []byte
	l, err = pr.readInt()
	if err != nil {
		return
	}
	if l > pr.maxPayloadSize {
		err = errPayloadTooLarge
		return
	}
	b = acquireByteBuffer(l)
	v, err = pr.read(l)
	if err != nil {
		releaseByteBuffer(b)
		b = nil
		return
	}
	b.Write(v)
	return
}

func (pr *protocolReader) readOp() (uint8, error) {
	return pr.readUint8()
}

func (pr *protocolReader) readFlags() (uint8, error) {
	return pr.readUint8()
}

func (pr *protocolReader) readHello() (version uint8, encodings []string, compressions []string, err error) {
	version, err = pr.readUint8()
	if err != nil {
		return
	}
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err != nil {
		return
	}
	settings := strings.Split(string(payload.Bytes()), "|")
	if len(settings) < 2 {
		err = errNotEnoughSettings
		return
	}
	if settings[0] != "" {
		encodings = strings.Split(settings[0], ",")
	}
	if settings[1] != "" {
		compressions = strings.Split(settings[1], ",")
	}
	releaseByteBuffer(payload)
	return
}

func (pr *protocolReader) readHelloAck() (pingInterval uint32, encoding string, compression string, err error) {
	var payload *bytes.Buffer
	pingInterval, err = pr.readUint32()
	if err != nil {
		return
	}
	payload, err = pr.readPayload()
	if err != nil {
		return
	}
	settings := strings.Split(string(payload.Bytes()), "|")
	if len(settings) < 2 {
		err = errNotEnoughSettings
		return
	}
	encoding = settings[0]
	compression = settings[1]
	releaseByteBuffer(payload)
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
	if err != nil {
		return
	}
	payload, err = pr.readPayload()
	return
}

func (pr *protocolReader) readResponse() (seq uint32, payload *bytes.Buffer, err error) {
	seq, err = pr.readUint32()
	if err != nil {
		return
	}
	payload, err = pr.readPayload()
	return
}

func (pr *protocolReader) readPush() (*bytes.Buffer, error) {
	return pr.readPayload()
}

func (pr *protocolReader) readError() (seq uint32, code uint16, reason string, err error) {
	seq, err = pr.readUint32()
	if err != nil {
		return
	}
	code, err = pr.readUint16()
	if err != nil {
		return
	}
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		reason = string(payload.Bytes())
	}
	releaseByteBuffer(payload)
	return
}

func (pr *protocolReader) readGoAway() (code uint16, reason string, err error) {
	code, err = pr.readUint16()
	if err != nil {
		return
	}
	var payload *bytes.Buffer
	payload, err = pr.readPayload()
	if err == nil {
		reason = string(payload.Bytes())
	}
	releaseByteBuffer(payload)
	return
}

func (pr *protocolReader) process(ph protocolHandler) error {
	op, err := pr.readOp()
	if err != nil {
		return err
	}
	flags, err := pr.readFlags()
	if err != nil {
		return err
	}
	switch op {
	case opHello:
		version, encodings, compressions, err := pr.readHello()
		if err != nil {
			return err
		}
		ph.handleHello(flags, version, encodings, compressions)
	case opHelloAck:
		pingInterval, encoding, compression, err := pr.readHelloAck()
		if err != nil {
			return err
		}
		ph.handleHelloAck(flags, pingInterval, encoding, compression)
	case opPing:
		seq, err := pr.readPing()
		if err != nil {
			return err
		}
		ph.handlePing(flags, seq)
	case opPong:
		seq, err := pr.readPong()
		if err != nil {
			return err
		}
		ph.handlePong(flags, seq)
	case opRequest:
		seq, payload, err := pr.readRequest()
		if err != nil {
			return err
		}
		ph.handleRequest(flags, seq, payload)
	case opResponse:
		seq, payload, err := pr.readResponse()
		if err != nil {
			return err
		}
		ph.handleResponse(flags, seq, payload)
	case opPush:
		payload, err := pr.readPush()
		if err != nil {
			return err
		}
		ph.handlePush(flags, payload)
	case opError:
		seq, code, reason, err := pr.readError()
		if err != nil {
			return err
		}
		ph.handleError(flags, seq, code, reason)
	case opGoAway:
		code, reason, err := pr.readGoAway()
		if err != nil {
			return err
		}
		ph.handleGoAway(flags, code, reason)
	default:
		return errUnknownOp
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

func (pw *protocolWriter) writeHello(flags uint8, version uint8, encodings []string, compressions []string) error {
	const headerLen = 7
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = opHello
	header[1:2][0] = flags
	header[2:3][0] = version
	settingsString := strings.Join([]string{
		strings.Join(encodings, ","),
		strings.Join(compressions, ","),
	}, "|")
	binary.BigEndian.PutUint32(header[3:headerLen], uint32(len(settingsString)))
	_, err := pw.Write(append(header[:headerLen], settingsString...))
	releaseByteBuffer(b)
	return err
}

func (pw *protocolWriter) writeHelloAck(flags uint8, pingInterval uint32, encoding string, compression string) error {
	const headerLen = 10
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = opHelloAck
	header[1:2][0] = flags
	binary.BigEndian.PutUint32(header[2:6], pingInterval)
	settingsString := strings.Join([]string{encoding, compression}, "|")
	binary.BigEndian.PutUint32(header[6:headerLen], uint32(len(settingsString)))
	_, err := pw.Write(append(header[:headerLen], settingsString...))
	releaseByteBuffer(b)
	return err
}

func (pw *protocolWriter) writePingPong(op uint8, flags uint8, seq uint32) error {
	const headerLen = 6
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = op
	header[1:2][0] = flags
	binary.BigEndian.PutUint32(header[2:headerLen], seq)
	_, err := pw.Write(header[:headerLen])
	releaseByteBuffer(b)
	return err
}

func (pw *protocolWriter) writePing(flags uint8, seq uint32) error {
	return pw.writePingPong(opPing, flags, seq)
}

func (pw *protocolWriter) writePong(flags uint8, seq uint32) error {
	return pw.writePingPong(opPong, flags, seq)
}

func (pw *protocolWriter) writeRequestResponse(op uint8, flags uint8, seq uint32, payload []byte) (err error) {
	const headerLen = 10
	b := acquireByteBuffer(headerLen + len(payload))
	header := b.Bytes()
	header[:1][0] = op
	header[1:2][0] = flags
	binary.BigEndian.PutUint32(header[2:6], seq)
	binary.BigEndian.PutUint32(header[6:headerLen], uint32(len(payload)))
	_, err = pw.Write(append(header[:headerLen], payload...))
	releaseByteBuffer(b)
	return
}

func (pw *protocolWriter) writeRequest(flags uint8, seq uint32, payload []byte) error {
	return pw.writeRequestResponse(opRequest, flags, seq, payload)
}

func (pw *protocolWriter) writeResponse(flags uint8, seq uint32, payload []byte) error {
	return pw.writeRequestResponse(opResponse, flags, seq, payload)
}

func (pw *protocolWriter) writePush(flags uint8, payload []byte) (err error) {
	const headerLen = 6
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = opPush
	header[1:2][0] = flags
	binary.BigEndian.PutUint32(header[2:headerLen], uint32(len(payload)))
	_, err = pw.Write(append(header[:headerLen], payload...))
	releaseByteBuffer(b)
	return
}

func (pw *protocolWriter) writeError(flags uint8, seq uint32, code uint16, reason string) (err error) {
	const headerLen = 12
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = opError
	header[1:2][0] = flags
	binary.BigEndian.PutUint32(header[2:6], seq)
	binary.BigEndian.PutUint16(header[6:8], code)
	binary.BigEndian.PutUint32(header[8:headerLen], uint32(len(reason)))
	_, err = pw.Write(append(header[:headerLen], reason...))
	releaseByteBuffer(b)
	return
}

func (pw *protocolWriter) writeGoAway(flags uint8, code uint16, reason string) (err error) {
	const headerLen = 8
	b := acquireByteBuffer(headerLen)
	header := b.Bytes()
	header[:1][0] = opGoAway
	header[1:2][0] = flags
	binary.BigEndian.PutUint16(header[2:4], code)
	binary.BigEndian.PutUint32(header[4:headerLen], uint32(len(reason)))
	_, err = pw.Write(append(header[:headerLen], reason...))
	releaseByteBuffer(b)
	return
}

type protocol struct {
	protocolReader
	protocolWriter
}

func newProtocol(rw io.ReadWriter, maxPayloadSize int) protocol {
	return protocol{
		newProtocolReader(rw, maxPayloadSize),
		newProtocolWriter(rw),
	}
}
