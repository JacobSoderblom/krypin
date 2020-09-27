package ticker

import (
	"context"
	"time"
)

func New(duration time.Duration, handler func(ctx context.Context)) *Ticker {
	return &Ticker{
		duration: duration,
		handler:  handler,
	}
}

type Ticker struct {
	duration time.Duration
	ticker   *time.Ticker
	handler  func(ctx context.Context)
}

func (t *Ticker) Execute(ctx context.Context) error {
	t.ticker = time.NewTicker(t.duration)

	for {
		select {
		case <-t.ticker.C:
			t.handler(ctx)
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

func (t *Ticker) Interrupt(err error) {
	t.ticker.Stop()
}
