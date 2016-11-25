package loqui

import (
	"bytes"
	"sync"
)

var byteBufferPool = sync.Pool{
	New: func() interface{} {
		return new(bytes.Buffer)
	},
}

func acquireByteBuffer(n int) *bytes.Buffer {
	b := byteBufferPool.Get().(*bytes.Buffer)
	b.Grow(n)
	return b
}

func releaseByteBuffer(b *bytes.Buffer) {
	if b == nil {
		return
	}
	b.Reset()
	byteBufferPool.Put(b)
}
