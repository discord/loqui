package loqui

import "sync"

type request struct {
	seq      uint32
	response *Response
	c        chan error
}

var requestPool = sync.Pool{
	New: func() interface{} {
		return &request{c: make(chan error)}
	},
}
