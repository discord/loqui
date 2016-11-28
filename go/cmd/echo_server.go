package main

import (
	"io"
	"github.com/hammerandchisel/loqui/go"
	"net/http"
)

type serverHandler []byte

func (s serverHandler) ServeRequest(ctx loqui.RequestContext) {
	io.CopyBuffer(ctx, ctx, nil)
}

func main() {
	defaultServer := loqui.NewServer(
		&serverHandler{},
		loqui.ServerConfig{SupportedEncodings: []string{"msgpack"}},
	)
	http.Handle("/_rpc", defaultServer)
	http.ListenAndServe(":8080", nil)

}