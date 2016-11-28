package loqui

import (
	"bufio"
	"bytes"
	"crypto/tls"
	"errors"
	"io"
	"io/ioutil"
	"net"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"
)

// ErrBadHandshake is returned when the server response to opening handshake is invalid.
var ErrBadHandshake = errors.New("loqui: bad handshake")

func hostPortNoPort(u *url.URL) (hostPort, hostNoPort string) {
	hostPort = u.Host
	hostNoPort = u.Host
	if i := strings.LastIndex(u.Host, ":"); i > strings.LastIndex(u.Host, "]") {
		hostNoPort = hostNoPort[:i]
	} else {
		switch u.Scheme {
		case "wss":
			hostPort += ":443"
		case "https":
			hostPort += ":443"
		default:
			hostPort += ":80"
		}
	}
	return hostPort, hostNoPort
}

// A Dialer contains options for connecting to a URL.
type Dialer struct {
	// SupportedEncodings contains a list of encodings that the client will
	// negotiate with the server.
	SupportedEncodings []string

	// SupportedCompressions contains a list of compressions that the client will
	// negotiate with the server.
	SupportedCompressions []string

	// MaxPayloadSize ensures the connection does not try to process any payload
	// larger than this size to avoid OOM from bad actors.
	MaxPayloadSize int

	// HandshakeTimeout specifies the duration for the handshake to complete.
	HandshakeTimeout time.Duration

	// Retries is the number of tries the user of the dialer should attempt.
	// The dialer itself does not do retries.
	Retries int
}

// Dial connects to a HTTP URL.
func (d *Dialer) Dial(urlString string) (*Conn, error) {
	u, err := url.Parse(urlString)
	if err != nil {
		return nil, err
	}

	req := &http.Request{
		Method:     "GET",
		URL:        u,
		Proto:      "HTTP/1.1",
		ProtoMajor: 1,
		ProtoMinor: 1,
		Header:     make(http.Header),
		Host:       u.Host,
	}

	req.Header["Upgrade"] = []string{"loqui"}
	req.Header["Connection"] = []string{"Upgrade"}

	hostPort, hostNoPort := hostPortNoPort(u)

	targetHostPort := hostPort

	var deadline time.Time
	if d.HandshakeTimeout != 0 {
		deadline = time.Now().Add(d.HandshakeTimeout)
	}

	netDialer := &net.Dialer{Deadline: deadline}
	netDial := netDialer.Dial

	netConn, err := netDial("tcp", targetHostPort)
	if err != nil {
		return nil, err
	}

	defer func() {
		if netConn != nil {
			netConn.Close()
		}
	}()

	if err := netConn.SetDeadline(deadline); err != nil {
		return nil, err
	}

	if u.Scheme == "https" {
		tlsConfig := tls.Config{
			ServerName: hostNoPort,
		}
		tlsConn := tls.Client(netConn, &tlsConfig)
		netConn = tlsConn
		if err := tlsConn.Handshake(); err != nil {
			return nil, err
		}
		if !tlsConfig.InsecureSkipVerify {
			if err := tlsConn.VerifyHostname(tlsConfig.ServerName); err != nil {
				return nil, err
			}
		}
	}

	br := bufio.NewReader(netConn)

	if err := req.Write(netConn); err != nil {
		return nil, err
	}

	resp, err := http.ReadResponse(br, req)
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != 101 ||
		!strings.EqualFold(resp.Header.Get("Upgrade"), "loqui") ||
		!strings.EqualFold(resp.Header.Get("Connection"), "upgrade") {
		resp.Body.Close()
		return nil, ErrBadHandshake
	}

	resp.Body = ioutil.NopCloser(bytes.NewReader([]byte{}))

	netConn.SetDeadline(time.Time{})

	conn := NewConn(br, netConn, netConn, ConnConfig{
		IsClient:              true,
		SupportedEncodings:    d.SupportedEncodings,
		SupportedCompressions: d.SupportedCompressions,
		MaxPayloadSize:        d.MaxPayloadSize,
	})

	if err := conn.Handshake(time.Now().Sub(deadline)); err != nil {
		return nil, err
	}

	netConn = nil // Avoid close in defer.

	return conn, nil
}

// Client automatically manages a connection with reconnects.
type Client struct {
	mu sync.RWMutex

	urlString string
	d         Dialer
	conn      *Conn
	backoff   *Backoff
}

// NewClient returns a Client.
func NewClient(urlString string, d Dialer) *Client {
	return &Client{
		d:         d,
		urlString: urlString,
		backoff:   NewBackoff(250*time.Millisecond, 2*time.Second),
	}
}

func (c *Client) dial() (conn *Conn, err error) {
	c.mu.RLock()
	conn = c.conn
	c.mu.RUnlock()

	if conn == nil {
		c.mu.Lock()
		defer c.mu.Unlock()

		// Double check that another goroutine didn't create the connection
		// while attempting to lock the mutex.
		conn = c.conn
		if conn != nil {
			return
		}

		c.conn, err = c.d.Dial(c.urlString)
		if err != nil {
			return
		}
	}

	c.backoff.Succeed()
	return
}

func (c *Client) maybeClear(conn *Conn, err error) bool {
	if err == nil {
		return false
	}

	// Request errors should not result in a new connection.
	if _, ok := err.(*RequestError); ok {
		return false
	}

	c.mu.Lock()
	defer c.mu.Unlock()

	// Don't clear if its not the same conn.
	if c.conn != conn {
		return false
	}

	c.conn = nil
	return true
}

func (c *Client) request(payload []byte, compressed bool, timeout time.Duration, retries int) (res io.ReadCloser, err error) {
	conn, err := c.dial()
	if err != nil {
		if retries > 0 {
			c.backoff.FailSleep()
			return c.request(payload, compressed, timeout, retries-1)
		}
		return
	}

	res, err = conn.RequestTimeout(payload, compressed, timeout)

	if c.maybeClear(conn, err) && retries > 0 {
		c.backoff.FailSleep()
		return c.request(payload, compressed, timeout, retries-1)
	}

	return
}

// Request provides a io.ReadCloser that contains the response payload.
// Close MUST be called to avoid allocations.
func (c *Client) Request(payload []byte, compressed bool, timeout time.Duration) (res io.ReadCloser, err error) {
	return c.request(payload, compressed, timeout, c.d.Retries)
}

// Push sends a payload to the other side of the connection.
func (c *Client) Push(payload []byte, compressed bool) (err error) {
	conn, err := c.dial()
	if err != nil {
		return
	}

	err = conn.Push(payload, compressed)

	// Pushes have no guarantees.
	// No retries are required but connection should be cleared for next call.
	c.maybeClear(conn, err)

	return
}
