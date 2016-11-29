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

func (wp workerPool) put(encoding string, compression string, compressed bool, seq uint32, payload *bytes.Buffer) {
	ctx := acquireRequestContext(payload)
	ctx.encoding = encoding
	ctx.compression = compression
	ctx.rcompressed = compressed
	ctx.seq = seq
	wp <- ctx
}

func (wp workerPool) run(c *Conn) {
	var seq uint32

	// If something panics then send an error for the last seq
	// and restart this goroutine.
	defer func() {
		if r := recover(); r != nil {
			reason, _ := r.(string)
			c.proto.writeError(FlagNone, seq, CodeInternalServerError, reason)
			go wp.run(c)
		}
	}()

	for ctx := range wp {
		seq = ctx.seq
		c.handler.ServeRequest(ctx)
		if !ctx.IsPush() {
			flags := FlagNone
			if ctx.wcompressed {
				flags = flags & FlagCompressed
			}
			c.proto.writeResponse(flags, seq, ctx.wbuf.Bytes())
		}
		releaseRequestContext(ctx)
	}
}
