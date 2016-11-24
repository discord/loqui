package loqui

import (
	"fmt"
	"log"
	"testing"
)

func BenchmarkClient(b *testing.B) {
	d := Dialer{
		SupportedEncodings: []string{"msgpack"},
	}
	url := fmt.Sprintf("http://127.0.0.1:%d/_rpc", 8080)
	conn, _ := d.Dial(url)

	payload := []byte("hello")

	b.ResetTimer()
	b.ReportAllocs()
	for i := 0; i < b.N; i++ {
		res, err := conn.Request(payload)
		if err != nil {
			log.Fatal(err)
		} else {
			res.Close()
		}
	}
}
