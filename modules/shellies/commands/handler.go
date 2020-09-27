package commands

import (
	"context"

	"github.com/JacobSoderblom/krypin/internal/core"
	"github.com/JacobSoderblom/krypin/internal/event"
)

var commandTopic = event.CommandTopic("shellies")

func Execute(ctx context.Context) error {
	log := core.Logger(ctx)

	log.Info("Staring commands")

	c := core.Subscribe(ctx, commandTopic)

	for {
		select {
		case ev := <-c:
			log.Info(ev)
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

func Interrupt(err error) {}
