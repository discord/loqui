package main

import (
	"fmt"
	"math/rand"
	"sync/atomic"
	"time"

	loqui "github.com/discordapp/loqui/go"
)

var state struct {
	i              int32
	failedRequests int32
	inFlight       int32
	maxAge         time.Duration
	requestTime    time.Duration
	values         [][]byte
}

func doWork(client *loqui.Conn) {
	startTime := time.Now()

	atomic.AddInt32(&state.inFlight, 1)
	v := state.values[rand.Intn(len(state.values))]
	_, err := client.Request(v, false)
	if err != nil {
		atomic.AddInt32(&state.failedRequests, 1)
	} else {
		atomic.AddInt32(&state.i, 1)
	}
	atomic.AddInt32(&state.inFlight, -1)

	age := time.Now().Sub(startTime)
	state.requestTime += age
	if age > state.maxAge {
		state.maxAge = age
	}
}

func logLoop() {
	lastI := int32(0)
	last := time.Now()

	for {
		time.Sleep(time.Second)

		now := time.Now()
		elapsed := now.Sub(last)
		reqSec := float64(state.i-lastI) / elapsed.Seconds()
		avgTime := time.Duration(0)
		i := atomic.LoadInt32(&state.i)
		if i-lastI > 0 {
			avgTime = time.Duration(float64(state.requestTime) / float64(state.i-lastI))
		}

		fmt.Printf("%d total requests (%.2f/sec). last log %.2f sec ago. %d failed, %d in flight, %s max, %s avg response time\n",
			i, reqSec, elapsed.Seconds(), state.failedRequests, state.inFlight, state.maxAge, avgTime)

		lastI = i
		last = now
		state.maxAge = 0
		state.requestTime = 0
	}
}

func workLoop(client *loqui.Conn) {
	fmt.Println("spawning work loop")
	for {
		doWork(client)
	}
}

var src = rand.NewSource(time.Now().UnixNano())

const letterBytes = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
const (
	letterIdxBits = 6                    // 6 bits to represent a letter index
	letterIdxMask = 1<<letterIdxBits - 1 // All 1-bits, as many as letterIdxBits
	letterIdxMax  = 63 / letterIdxBits   // # of letter indices fitting in 63 bits
)

func randBytes(n int) []byte {
	b := make([]byte, n)
	for i, cache, remain := n-1, src.Int63(), letterIdxMax; i >= 0; {
		if remain == 0 {
			cache, remain = src.Int63(), letterIdxMax
		}
		if idx := int(cache & letterIdxMask); idx < len(letterBytes) {
			b[i] = letterBytes[idx]
			i--
		}
		cache >>= letterIdxBits
		remain--
	}

	return b
}

func main() {
	d := loqui.Dialer{
		SupportedEncodings: []string{"msgpack"},
	}
	client, err := d.Dial("http://127.0.0.1:8080/_rpc")
	if err != nil {
		panic(err)
	}

	// state.values = [][]byte{[]byte("hello world")}
	state.values = make([][]byte, 1000)
	for i := 0; i < len(state.values); i++ {
		state.values[i] = randBytes(1024 + rand.Intn(1024*50))
	}

	for i := 0; i < 100; i++ {
		go workLoop(client)
	}

	logLoop()
}
