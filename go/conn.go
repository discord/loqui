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

	// Client
	ready   bool
	readyCh chan struct{}

	reqs map[uint32]*request

	// Config
	isClient           bool
	version            uint8
	pingInterval       time.Duration
	supportedEncodings []string
	encoding           string
}

// NewConn creates a connection instance out of any ReadWriteCloser combo.
//
// Behavior changes depending on side of the connection using the isClient option.
func NewConn(r io.Reader, w io.Writer, c io.Closer, isClient bool) (conn *Conn) {
	writeCh := make(writeChan, 1000)

	conn = &Conn{
		isClient: isClient,

		// Writes are buffered and flushed by the writeLoop to avoid excessive system calls.
		w:       bufio.NewWriterSize(w, 65536),
		writeCh: writeCh,

		c: c,

		proto: protocol{
			newProtocolReader(r),
			newProtocolWriter(writeCh),
		},

		// Signal channels that are never sent to and just closed to coordinate goroutines.
		readyCh:     make(chan struct{}),
		terminateCh: make(chan struct{}),

		pongCh:    make(chan uint32, 1),
		readErrCh: make(chan error, 1),
	}

	if isClient {
		conn.reqs = make(map[uint32]*request)
	} else {
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
		if err := c.proto.writePing(seq); err != nil {
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
			c.w.ReadFrom(b)
			releaseByteBuffer(b)
			// Drain the write channel to to reduce syscalls.
		drain:
			for {
				select {
				case b = <-c.writeCh:
					c.w.ReadFrom(b)
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

func (c *Conn) selectEncoding(encodings []string) bool {
	for _, supportedEncoding := range c.supportedEncodings {
		for _, encoding := range encodings {
			if encoding == supportedEncoding {
				c.encoding = encoding
				c.proto.writeSelectEncoding(encoding)
				return true
			}
		}
	}
	return false
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

// AwaitReady ensures that the server has responded with hello.
func (c *Conn) AwaitReady(timeout time.Duration) (err error) {
	if !c.isClient {
		return ErrNotClient
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
func (c *Conn) Request(payload []byte) (io.ReadCloser, error) {
	return c.RequestTimeout(payload, time.Second*5)
}

// RequestTimeout provides a io.ReadCloser that contains the response payload.
// Close MUST be called to avoid allocations.
func (c *Conn) RequestTimeout(payload []byte, timeout time.Duration) (res io.ReadCloser, err error) {
	if !c.isClient {
		return nil, ErrNotClient
	}

	if c.localClosed || c.remoteClosed {
		return nil, io.EOF
	}

	if !c.ready {
		return nil, ErrNotReady
	}

	tc := acquireTimer(timeout)
	req := c.acquireRequest()
	err = c.proto.writeRequest(req.seq, payload)
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
func (c *Conn) Push(payload []byte) (err error) {
	if c.localClosed || c.remoteClosed {
		return io.EOF
	}

	if !c.ready {
		return ErrNotReady
	}

	return c.proto.writePush(payload)
}

// Serve spawns a pool of workers to handle requests and pushes.
// Does not return until connection is terminated.
func (c *Conn) Serve(concurrency int) (err error) {
	if err = c.proto.writeHello(uint32(c.pingInterval.Seconds()*1000), c.supportedEncodings); err != nil {
		return
	}

	c.wp.start(c, concurrency)

	return <-c.readErrCh
}

// Close performs a graceful shutdown of the connection by sending a GoAway.
func (c *Conn) Close(code uint16) (err error) {
	if c.localClosed {
		return nil
	}

	c.localClosed = true
	return c.proto.writeGoAway(code, "")
}

// Terminate sends a GoAway and immediately terminates the underlying connection.
func (c *Conn) Terminate(code uint16) (err error) {
	err = c.Close(code)
	if err != nil {
		return
	}
	return c.terminate()
}

// ProtocolHandler

func (c *Conn) handleHello(version uint8, pingInterval uint32, encodings []string) {
	if !c.isClient {
		c.Terminate(CodeInvalidOp)
		return
	}

	if !c.selectEncoding(encodings) {
		c.Terminate(CodeInvalidEncoding)
		return
	}

	if c.ready {
		c.Terminate(CodeInvalidOp)
		return
	}

	c.version = version
	c.pingInterval = time.Duration(pingInterval) * time.Millisecond
	c.ready = true
	close(c.readyCh)
}

func (c *Conn) handleSelectEncoding(encoding string) {
	if c.ready {
		c.Terminate(CodeInvalidOp)
		return
	}

	for _, supportedEncoding := range c.supportedEncodings {
		if supportedEncoding == encoding {
			c.encoding = encoding
			c.ready = true
			close(c.readyCh)
			return
		}
	}

	c.Terminate(CodeInvalidEncoding)
}

func (c *Conn) handlePing(seq uint32) {
	c.proto.writePong(seq)
}

func (c *Conn) handlePong(seq uint32) {
	c.pongCh <- seq
}

func (c *Conn) handleRequest(seq uint32, payload *bytes.Buffer) {
	if c.isClient {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.wp.put(c.encoding, seq, payload)
}

func (c *Conn) handleResponse(seq uint32, payload *bytes.Buffer) {
	if !c.isClient {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.completeRequest(seq, payload, nil)
}

func (c *Conn) handlePush(payload *bytes.Buffer) {
	if c.isClient {
		releaseByteBuffer(payload)
		c.Terminate(CodeInvalidOp)
		return
	}

	c.wp.put(c.encoding, 0, payload)
}

func (c *Conn) handleError(seq uint32, code uint16, reason string) {
	c.completeRequest(seq, nil, errors.New(reason))
}

func (c *Conn) handleGoAway(code uint16, reason string) {
	c.remoteClosed = true

	if c.localClosed {
		c.Terminate(CodeNormal)
	}
}
