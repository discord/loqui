package loqui

import (
	"bytes"
	"sync"
)

var bufferPool = sync.Pool{
	New: func() interface{} {
		return new(bytes.Buffer)
	},
}

func acquireBuffer(n int) *bytes.Buffer {
	buf := bufferPool.Get().(*bytes.Buffer)
	buf.Reset()
	buf.Grow(n)
	return buf
}

func releaseBuffer(buf *bytes.Buffer) {
	if buf == nil {
		return
	}
	bufferPool.Put(buf)
}
