package pubsub_transport

import (
	"context"

	"github.com/JacobSoderblom/krypin/pkg/log"

	"github.com/JacobSoderblom/krypin/devicereg/transport"

	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/JacobSoderblom/krypin/internal/event"
	"github.com/JacobSoderblom/krypin/internal/pubsub"
	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
)

func NewPubSubActor(handlers transport.PubSubHandlerSet, client *mqtt.Client, db *database.Database, errLogger log.Logger) (func(context.Context) error, func(error)) {
	act := pubsub.New(client)

	act.Handle(event.DiscoveredTopic, handlers.Discover, ErrorLogger(errLogger), database.PubSubTransaction(db))
	act.Handle(event.EntityStateTopic, handlers.EntityStateUpdate, ErrorLogger(errLogger), database.PubSubTransaction(db))

	return act.Execute, act.Interrupt
}

func ErrorLogger(errLogger log.Logger) pubsub.Middleware {
	return func(next pubsub.Handler) pubsub.Handler {
		return func(ctx context.Context, ev *event.Event, publisher pubsub.Publish) (err error) {
			defer func() {
				if err != nil {
					errLogger.Log("error", err)
				}
			}()

			return next(ctx, ev, publisher)
		}
	}
}
