package loqui

import (
	"bufio"
	"bytes"
	"errors"
	"io"
	"sync"
	"sync/atomic"
	"time"
)

var (
	// ErrTimeout is returned from timed out calls.
	ErrTimeout = errors.New("loqui: timeout")

	// ErrNotClient is returned from calls that only client can make.
	ErrNotClient = errors.New("loqui: not client")

	// ErrNotReady is returned when issuing calls before handshake is complete.
	ErrNotReady = errors.New("loqui: not ready")
)

// ConnConfig configures a Conn.
type ConnConfig struct {
	IsClient              bool
	Handler               ServerHandler
	PingInterval          time.Duration
	SupportedEncodings    []string
	SupportedCompressions []string
	MaxPayloadSize        int
}

// Conn supports client/server of the protocol.
type Conn struct {
	mu sync.Mutex

	proto protocol
	seq   uint32

	localClosed  bool
	remoteClosed bool
	terminated   bool
	terminateCh  chan struct{}

	w       *bufio.Writer
	writeCh writeChan

	c io.Closer

	pongCh chan uint32

	// Server
	handler   ServerHandler
	wp        workerPool
	readErrCh chan error
	serving   bool

	// Client
	ready   bool
	readyCh chan struct{}

	reqs map[uint32]*request

	// Config
	isClient              bool
	version               uint8
	pingInterval          time.Duration
	supportedEncodings    []string
	supportedCompressions []string
	encoding              string
	compression           string
}

// NewConn creates a connection instance out of any ReadWriteCloser combo.
//
// Behavior changes depending on side of the connection using the isClient option.
func NewConn(r io.Reader, w io.Writer, c io.Closer, config ConnConfig) (conn *Conn) {
	writeCh := make(writeChan, 1000)

	conn = &Conn{
		version: version,

		isClient:              config.IsClient,
		pingInterval:          config.PingInterval,
		handler:               config.Handler,
		supportedEncodings:    config.SupportedEncodings,
		supportedCompressions: config.SupportedCompressions,

		// Writes are buffered and flushed by the writeLoop to avoid excessive system calls.
		w:       bufio.NewWriterSize(w, 65536),
		writeCh: writeCh,

		c: c,

		proto: protocol{
			newProtocolReader(r, config.MaxPayloadSize),
			newProtocolWriter(writeCh),
		},

		// Signal channels that are never sent to and just closed to coordinate goroutines.
		readyCh:     make(chan struct{}),
		terminateCh: make(chan struct{}),

		pongCh:    make(chan uint32, 1),
		readErrCh: make(chan error, 1),
	}

	if conn.isClient {
		conn.reqs = make(map[uint32]*request)
	}

	if conn.handler != nil {
		conn.wp = newWorkerPool()
	}

	go conn.pingLoop()
	go conn.readLoop()
	go conn.writeLoop()

	return
}

func (c *Conn) pingLoop() {
	select {
	case <-c.terminateCh:
		return
	case <-c.readyCh:
	}

	for {
		seq := c.nextSeq()
		if err := c.proto.writePing(FlagNone, seq); err != nil {
			return
		}

		select {
		case <-c.terminateCh:
			return
		case <-time.After(c.pingInterval):
		}

		select {
		case <-c.pongCh:
		default:
			c.Terminate(CodePingTimeout)
			return
		}
	}
}

func (c *Conn) readLoop() {
	for {
		err := c.proto.process(c)
		if err != nil {
			c.readErrCh <- err
			c.terminate()
			return
		}
	}
}

func (c *Conn) writeLoop() {
	for {
		select {
		case <-c.terminateCh:
			return
		case b := <-c.writeCh:
			c.w.Write(b.Bytes())
			releaseByteBuffer(b)
			// Drain the write channel to to reduce syscalls.
		drain:
			for {
				select {
				case b = <-c.writeCh:
					c.w.Write(b.Bytes())
					releaseByteBuffer(b)
				default:
					break drain
				}
			}
			c.w.Flush()
		}
	}
}

func (c *Conn) acquireRequest() *request {
	req := requestPool.Get().(*request)
	req.seq = c.nextSeq()
	c.mu.Lock()
	c.reqs[req.seq] = req
	c.mu.Unlock()
	return req
}

func (c *Conn) releaseRequest(req *request) {
	c.mu.Lock()
	delete(c.reqs, req.seq)
	c.mu.Unlock()
	requestPool.Put(req)
}

func (c *Conn) nextSeq() uint32 {
	return atomic.AddUint32(&c.seq, 1)
}

func (c *Conn) sendHelloAck(encodings []string, compressions []string) bool {
compressions:
	for _, supportedEncoding := range c.supportedCompressions {
		for _, compression := range compressions {
			if compression == supportedEncoding {
				c.compression = compression
				break compressions
			}
		}
	}

encodings:
	for _, supportedEncoding := range c.supportedEncodings {
		for _, encoding := range encodings {
			if encoding == supportedEncoding {
				c.encoding = encoding
				break encodings
			}
		}
	}

	if c.encoding == "" {
		return false
	}

	c.proto.writeHelloAck(FlagNone, uint32(c.pingInterval.Seconds()*1000), c.encoding, c.compression)
	return true
}

func (c *Conn) completeRequest(seq uint32, payload *bytes.Buffer, err error) {
	c.mu.Lock()
	req, ok := c.reqs[seq]
	c.mu.Unlock()

	if ok {
		res := responsePool.Get().(*Response)
		res.payload = payload

		req.response = res
		req.c <- err

		if c.remoteClosed {
			c.mu.Lock()
			noReqs := len(c.reqs) == 0
			c.mu.Unlock()
			if noReqs {
				c.Terminate(CodeNormal)
			}
		}
	} else if err == nil {
		releaseByteBuffer(payload)
	}
}

func (c *Conn) terminate() (err error) {
	if c.terminated {
		return
	}

	c.localClosed = true
	c.remoteClosed = true
	c.terminated = true
	close(c.terminateCh)

	c.mu.Lock()
	for _, req := range c.reqs {
		req.c <- io.EOF
	}
	c.mu.Unlock()

	return c.c.Close()
}

// Public

// Handshake ensures that the server has responded with HELLO_ACK.
func (c *Conn) Handshake(timeout time.Duration) (err error) {
	if !c.isClient {
		return ErrNotClient
	}

	err = c.proto.writeHello(FlagNone, c.version, c.supportedEncodings, c.supportedCompressions)
	if err != nil {
		return
	}

	if !c.ready {
		tc := acquireTimer(timeout)
		select {
		case <-c.readyCh:
		case <-c.terminateCh:
			err = io.EOF
		case <-tc.C:
			err = ErrTimeout
		}
		releaseTimer(tc)
	}

	return nil
}

// Serve spawns a pool of workers to handle requests and pushes.
// Does not return until connection is terminated.
func (c *Conn) Serve(concurrency int) (err error) {
	if c.handler == nil {
		panic("handler is requried")
	}

	c.wp.start(c, concurrency)
	defer c.wp.stop()

	c.serving = true
	err = <-c.readErrCh
	c.serving = false
	return
}

// Encoding returns the negotiated encoding.
// If connection is not ready it waits.
func (c *Conn) Encoding() (encoding string, err error) {
	if !c.ready {
		err = ErrNotReady
	} else {
		encoding = c.encoding
	}
	return
}

// Request provides a io.ReadCloser that contains the response payload.
// Close MUST be called to avoid allocations.
func (c *Conn) Request(payload []byte, compressed bool) (io.ReadCloser, error) {
	return c.RequestTimeout(payload, compressed, time.Second*5)
}

// RequestTimeout provides a io.ReadCloser that contains the response payload.
// Close MUST be called to avoid allocations.
func (c *Conn) RequestTimeout(payload []byte, compressed bool, timeout time.Duration) (res io.ReadCloser, err error) {
	if !c.isClient {
		return nil, ErrNotClient
	}

	if c.localClosed || c.remoteClosed {
		return nil, io.EOF
	}

	if !c.ready {
		return nil, ErrNotReady
	}

	flags := FlagNone
	if compressed {
		flags = flags & FlagCompressed
	}

	tc := acquireTimer(timeout)
	req := c.acquireRequest()
	err = c.proto.writeRequest(flags, req.seq, payload)
	if err == nil {
		select {
		case err = <-req.c:
			res = req.response
		case <-tc.C:
			err = ErrTimeout
		}
	}
	c.releaseRequest(req)
	releaseTimer(tc)

	return
}

// Push sends a payload to the other side of the connection.
// There is no backpressure and the only errors that can return are socket errors.
func (c *Conn) Push(payload []byte, compressed bool) (err error) {
	if c.localClosed || c.remoteClosed {
		return io.EOF
	}

	if !c.ready {
		return ErrNotReady
	}

	flags := FlagNone
	if compressed {
		flags = flags & FlagCompressed
	}

	return c.proto.writePush(flags, payload)
}

// Close performs a graceful shutdown of the connection by sending a GoAway.
func (c *Conn) Close(code uint16) (err error) {
	if c.localClosed {
		return nil
	}

	c.localClosed = true
	return c.proto.writeGoAway(FlagNone, code, "")
}

// Terminate sends a GoAway and immediately terminates the underlying connection.
func (c *Conn) Terminate(code uint16) (err error) {
	err = c.Close(code)
	if err != nil {
		return
	}
	return c.terminate()
}

// Closed returns if the connection is closed on either side.
func (c *Conn) Closed() bool {
	return c.localClosed || c.remoteClosed
}

// ProtocolHandler

func (c *Conn) handleHello(flags uint8, version uint8, encodings []string, compressions []string) {
	if c.isClient || c.ready {
		c.Terminate(CodeInvalidOp)
		return
	}

	if c.version < version {
		c.Terminate(CodeUnsupportedVersion)
		return
	}

	if !c.sendHelloAck(encodings, compressions) {
		c.Terminate(CodeNoCommonEncoding)
		return
	}

	c.version = version
	c.ready = true
	close(c.readyCh)
}

func (c *Conn) handleHelloAck(flags uint8, pingInterval uint32, encoding string, compression string) {
	if !c.isClient || c.ready {
		c.Terminate(CodeInvalidOp)
		return
	}

	c.pingInterval = time.Duration(pingInterval) * time.Millisecond

	validCompression := compression == ""
	if !validCompression {
		for _, supportedCompression := range c.supportedCompressions {
			if supportedCompression == compression {
				c.compression = compression
				validCompression = true
				break
			}
		}
	}

	validEncoding := false
	for _, supportedEncoding := range c.supportedEncodings {
		if supportedEncoding == encoding {
			c.encoding = encoding
			validEncoding = true
			break
		}
	}

	if !validEncoding {
		c.Terminate(CodeInvalidEncoding)
		return
	}

	if !validCompression {
		c.Terminate(CodeInvalidCompression)
		return
	}

	c.ready = true
	close(c.readyCh)
}

func (c *Conn) handlePing(flags uint8, seq uint32) {
	c.proto.writePong(FlagNone, seq)
}

func (c *Conn) handlePong(flags uint8, seq uint32) {
	c.pongCh <- seq
}

func (c *Conn) handleRequest(flags uint8, seq uint32, payload *bytes.Buffer) {
	if c.isClient {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.wp.put(c.encoding, c.compression, flags&FlagCompressed == FlagCompressed, seq, payload)
}

func (c *Conn) handleResponse(flags uint8, seq uint32, payload *bytes.Buffer) {
	if !c.isClient {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.completeRequest(seq, payload, nil)
}

func (c *Conn) handlePush(flags uint8, payload *bytes.Buffer) {
	if !c.serving {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.wp.put(c.encoding, c.compression, flags&FlagCompressed == FlagCompressed, 0, payload)
}

func (c *Conn) handleError(flags uint8, seq uint32, code uint16, reason string) {
	c.completeRequest(seq, nil, &RequestError{code, reason})
}

func (c *Conn) handleGoAway(flags uint8, code uint16, reason string) {
	c.remoteClosed = true

	if c.localClosed {
		c.Terminate(CodeNormal)
	}
}

// RequestError to a request when an OP_ERROR arrives.
type RequestError struct {
	Code   uint16
	Reason string
}

func (err *RequestError) Error() string {
	return err.Reason
}
