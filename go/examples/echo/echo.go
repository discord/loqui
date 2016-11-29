package main

import (
	"io"
	"net/http"

	"github.com/hammerandchisel/loqui/go"
)

type serverHandler []byte

func (s serverHandler) ServeRequest(ctx loqui.RequestContext) {
	io.CopyBuffer(ctx, ctx, s)
}

func main() {
	defaultServer := loqui.NewServer(
		make(serverHandler, 4096),
		loqui.ServerConfig{SupportedEncodings: []string{"msgpack"}},
	)
	http.Handle("/_rpc", defaultServer)
	http.ListenAndServe(":8080", nil)
}
