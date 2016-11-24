package main

import (
	"io"
	"net/http"
	"sync"

	loqui "github.com/hammerandchisel/loqui/go"
)

type RequestHandler struct{}

var pool = sync.Pool{
	New: func() interface{} {
		return make([]byte, 4096)
	},
}

func (rh *RequestHandler) ServeRequest(ctx loqui.RequestContext) {
	data := pool.Get().([]byte)
	io.CopyBuffer(ctx, ctx, data)
	pool.Put(data)
}

func main() {
	http.Handle("/_rpc", loqui.NewServer(new(RequestHandler), loqui.ServerConfig{
		SupportedEncodings: []string{"msgpack"},
	}))

	http.ListenAndServe(":8080", nil)
}
