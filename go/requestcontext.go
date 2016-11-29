package loqui

import (
	"bytes"
	"net/http"
	"strings"
	"sync"
)

// RequestContext contains incoming request and manages outgoing response.
type RequestContext interface {
	IsPush() bool
	Write([]byte) (int, error)
	Read(p []byte) (n int, err error)
	Encoding() string
	Compression() string
	ReadCompressed() bool
	SetWriteCompressed(bool)
}

type requestContext struct {
	wbuf        *bytes.Buffer
	wcompressed bool
	rbuf        *bytes.Buffer
	rcompressed bool
	seq         uint32
	encoding    string
	compression string
}

func (ctx *requestContext) Write(p []byte) (n int, err error) {
	return ctx.wbuf.Write(p)
}

func (ctx *requestContext) Read(p []byte) (n int, err error) {
	return ctx.rbuf.Read(p)
}

func (ctx *requestContext) Seq() uint32 {
	return ctx.seq
}

func (ctx *requestContext) IsPush() bool {
	return ctx.seq == 0
}

func (ctx *requestContext) Encoding() string {
	return ctx.encoding
}

func (ctx *requestContext) Compression() string {
	return ctx.compression
}

func (ctx *requestContext) ReadCompressed() bool {
	return ctx.rcompressed
}

func (ctx *requestContext) SetWriteCompressed(compressed bool) {
	ctx.wcompressed = compressed
}

var requestContextPool = sync.Pool{
	New: func() interface{} {
		return &requestContext{wbuf: new(bytes.Buffer)}
	},
}

func acquireRequestContext(rbuf *bytes.Buffer) *requestContext {
	ctx := requestContextPool.Get().(*requestContext)
	ctx.rbuf = rbuf
	ctx.wbuf.Reset()
	return ctx
}

func releaseRequestContext(ctx *requestContext) {
	releaseByteBuffer(ctx.rbuf)
	ctx.rbuf = nil
	requestContextPool.Put(ctx)
}

type httpRequestContext struct {
	w        http.ResponseWriter
	req      *http.Request
	encoding string
}

func newHTTPRequestContext(w http.ResponseWriter, req *http.Request) RequestContext {
	return &httpRequestContext{
		w:        w,
		req:      req,
		encoding: strings.TrimPrefix(req.Header.Get("Content-Type"), "application/"),
	}
}

func (ctx *httpRequestContext) Write(p []byte) (n int, err error) {
	ctx.w.Header().Set("Content-Type", "application/"+ctx.encoding)
	return ctx.w.Write(p)
}

func (ctx *httpRequestContext) Read(p []byte) (n int, err error) {
	return ctx.req.Body.Read(p)
}

func (ctx *httpRequestContext) Seq() uint32 {
	return 0
}

func (ctx *httpRequestContext) IsPush() bool {
	return false
}

func (ctx *httpRequestContext) Encoding() string {
	return ctx.encoding
}

func (ctx *httpRequestContext) Compression() string {
	return ""
}

func (ctx *httpRequestContext) ReadCompressed() bool {
	return false
}

func (ctx *httpRequestContext) SetWriteCompressed(compressed bool) {
}
