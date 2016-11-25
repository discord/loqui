package loqui

import "bytes"

// writeChan implements an io.Writer that allocates buffers and sends them into the channel.
type writeChan chan *bytes.Buffer

func (ch writeChan) Write(p []byte) (n int, err error) {
	b := acquireByteBuffer(len(p))
	n, err = b.Write(p)
	ch <- b
	return
}
