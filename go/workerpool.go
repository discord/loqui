package loqui

import "bytes"

type workerPool chan *requestContext

func newWorkerPool() workerPool {
	return make(chan *requestContext)
}

func (wp workerPool) start(c *Conn, concurrency int) {
	for i := 0; i < concurrency; i++ {
		go wp.run(c)
	}
}

func (wp workerPool) stop() {
	close(wp)
}

func (wp workerPool) put(encoding string, seq uint32, payload *bytes.Buffer) {
	ctx := acquireRequestContext(payload)
	ctx.encoding = encoding
	ctx.seq = seq
	wp <- ctx
}

func (wp workerPool) run(c *Conn) {
	for ctx := range wp {
		c.handler.ServeRequest(ctx)
		if !ctx.IsPush() {
			c.proto.writeResponse(ctx.seq, ctx.wbuf.Bytes())
		}
		releaseRequestContext(ctx)
	}
}
