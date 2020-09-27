package events

import (
	"context"
)

var eventsToSubscribe = map[string]byte{
	"shellies/+/info": 1,
}

func Execute(ctx context.Context) error {

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

func Interrupt(err error) {}
