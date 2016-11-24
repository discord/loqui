package loqui

import (
	"bytes"
	"sync"
)

// Response carries the payload.
type Response struct {
	payload *bytes.Buffer
}

func (r Response) Read(p []byte) (n int, err error) {
	return r.payload.Read(p)
}

// Close releases response back to the response pool.
func (r *Response) Close() error {
	releaseBuffer(r.payload)
	r.payload = nil
	responsePool.Put(r)
	return nil
}

var responsePool = sync.Pool{
	New: func() interface{} {
		return new(Response)
	},
}
