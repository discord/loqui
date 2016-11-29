package main

import (
	"bytes"
	"io"
	"net/http"
	"sync"

	"github.com/hammerandchisel/loqui/go"
)

var pool = sync.Pool{
	New: func() interface{} {
		return bytes.NewBuffer(make([]byte, 1024*100))
	},
}

type serverHandler []byte

func (s serverHandler) ServeRequest(ctx loqui.RequestContext) {
	v := pool.Get().(*bytes.Buffer)
	io.CopyBuffer(ctx, ctx, v.Bytes())
	pool.Put(v)
}

func main() {
	defaultServer := loqui.NewServer(
		make(serverHandler, 4096),
		loqui.ServerConfig{SupportedEncodings: []string{"msgpack"}},
	)
	http.Handle("/_rpc", defaultServer)
	http.ListenAndServe(":8080", nil)
}
