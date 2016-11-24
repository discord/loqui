package loqui

import (
	"io"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"
)

type ServerHandler interface {
	ServeRequest(ctx RequestContext)
}

type ServerConfig struct {
	PingInterval       time.Duration
	SupportedEncodings []string
	Concurrency        int
}

type Server struct {
	mu      sync.Mutex
	conns   map[*Conn]bool
	handler ServerHandler
	config  ServerConfig
}

func NewServer(handler ServerHandler, config ServerConfig) *Server {
	if config.PingInterval == 0 {
		config.PingInterval = time.Millisecond * 30000
	}

	if config.Concurrency == 0 {
		config.Concurrency = 50
	}

	return &Server{
		conns:   make(map[*Conn]bool),
		handler: handler,
		config:  config,
	}
}

func (s *Server) upgrade(w http.ResponseWriter, req *http.Request) {
	if req.Method != "GET" || !strings.EqualFold(req.Header.Get("Upgrade"), "loqui") {
		w.WriteHeader(http.StatusUpgradeRequired)
		return
	}

	conn, _, err := w.(http.Hijacker).Hijack()
	if err != nil {
		return
	}
	io.WriteString(conn, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: loqui\r\nConnection: Upgrade\r\n\r\n")

	s.serveConn(conn)
}

func (s *Server) ServeHTTP(w http.ResponseWriter, req *http.Request) {
	if req.Method == "POST" {
		s.handler.ServeRequest(newHTTPRequestContext(w, req))
	} else {
		s.upgrade(w, req)
	}
}

func (s *Server) serveConn(conn net.Conn) (err error) {
	c := NewConn(conn, conn, conn, false)
	c.pingInterval = s.config.PingInterval
	c.handler = s.handler
	c.supportedEncodings = s.config.SupportedEncodings

	s.mu.Lock()
	s.conns[c] = true
	s.mu.Unlock()
	defer func() {
		s.mu.Lock()
		delete(s.conns, c)
		s.mu.Unlock()
	}()

	return c.Serve(s.config.Concurrency)
}
