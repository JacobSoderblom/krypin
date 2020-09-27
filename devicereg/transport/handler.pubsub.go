package transport

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/JacobSoderblom/krypin/internal/errors"
	"github.com/JacobSoderblom/krypin/pkg/websocket"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/internal/event"
	"github.com/JacobSoderblom/krypin/internal/pubsub"
)

type PubSubHandlerSet struct {
	Discover          pubsub.Handler
	EntityStateUpdate pubsub.Handler
}

func PubSubHandlers(svc devicereg.Service, socket websocket.Broadcaster) PubSubHandlerSet {
	return PubSubHandlerSet{
		Discover:          Discover(svc, socket),
		EntityStateUpdate: EntityStateUpdate(svc),
	}
}

func Discover(svc devicereg.Service, socket websocket.Broadcaster) pubsub.Handler {
	return func(ctx context.Context, ev *event.Event, publisher pubsub.Publish) error {
		d := devicereg.Device{}
		err := ev.GetPayload(&d)
		if err != nil {
			return errors.New(errors.Internal, fmt.Sprintf("could not parse event payload as device: %s", err.Error()))
		}

		device, err := svc.Add(ctx, d)
		if errors.Is(errors.Conflict, err) {
			// do nothing if we already have that device
			return nil
		}

		if err != nil {
			return err
		}

		b, err := json.Marshal(websocket.Response{
			Topic:  "devices/discovered",
			Result: device,
		})
		if err != nil {
			return errors.New(errors.Internal, fmt.Sprintf("could not marshal websocket response with device: %v", err.Error()))
		}

		return socket.Broadcast(b)
	}
}

func EntityStateUpdate(svc devicereg.Service) pubsub.Handler {
	return func(ctx context.Context, ev *event.Event, publisher pubsub.Publish) error {
		states := []*devicereg.EntityState{}

		err := ev.GetPayload(&states)
		if err != nil {
			return errors.New(errors.Internal, fmt.Sprintf("could not parse event payload as entities: %s", err.Error()))
		}

		_, err = svc.AddEntityStates(ctx, states...)
		if err != nil {
			return err
		}

		return nil
	}
}
