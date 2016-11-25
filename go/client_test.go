package loqui

import "testing"

func BenchmarkClient(b *testing.B) {
	client, _ := newPair()

	if _, err := client.Encoding(); err != nil {
		b.Fatalf("unexpected error: %s", err)
	}

	payload := []byte("hello world")

	b.ResetTimer()
	b.ReportAllocs()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			res, err := client.Request(payload)
			if err != nil {
				b.Fatalf("unexpected error: %s", err)
			} else {
				res.Close()
			}
		}
	})
}
