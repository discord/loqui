package loqui

import (
	"math"
	"math/rand"
	"time"
)

// Backoff manages an exponential backoff.
type Backoff struct {
	min     time.Duration
	max     time.Duration
	current time.Duration
	fails   int
}

// NewBackoff returns a new backoff.
func NewBackoff(minDelay time.Duration, maxDelay time.Duration) *Backoff {
	return &Backoff{
		min:     minDelay,
		max:     maxDelay,
		current: minDelay,
	}
}

// Succeed resets the backoff.
func (backoff *Backoff) Succeed() {
	backoff.fails = 0
	backoff.current = backoff.min
}

// Fail increments the backoff and returns the delay to wait.
func (backoff *Backoff) Fail() time.Duration {
	backoff.fails++
	delay := float64(backoff.current*2) * rand.Float64()

	current := float64(backoff.current)
	current += delay

	if backoff.max > 0 {
		current = math.Min(current, float64(backoff.max))
	}

	backoff.current = time.Duration(current)

	return backoff.current
}

// FailSleep increments the backoff and sleeps the new amount of time.
func (backoff *Backoff) FailSleep() {
	time.Sleep(backoff.Fail())
}
