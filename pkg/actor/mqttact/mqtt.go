package mqttact

import (
	"context"
	"regexp"
	"strings"

	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
)

type Handler func(context.Context, mqtt.Message, mqtt.Publish) error
type Middleware func(next Handler) Handler

func New(client *mqtt.Client) *Actor {
	return &Actor{
		mqtt:     client,
		handlers: map[string]Handler{},
		topics:   map[string]byte{},
	}
}

type Actor struct {
	mqtt     *mqtt.Client
	handlers map[string]Handler
	topics   map[string]byte
}

func (a *Actor) Handle(topic string, handlerFn Handler, middlwares ...Middleware) {
	handler := handlerFn

	for _, mw := range middlwares {
		handler = mw(handler)
	}

	a.handlers[topic] = handler
	a.topics[topic] = 0
}

func (a *Actor) Execute(ctx context.Context) error {
	msgChan := a.mqtt.SubscribeMultiple(a.topics)

	for {
		select {
		case msg := <-msgChan:
			for topic, handler := range a.handlers {
				foundOneLevel, _ := regexp.MatchString(strings.ReplaceAll(topic, "+", "([a-zA-Z0-9 _.-]+)"), msg.Topic())

				if !foundOneLevel {
					continue
				}

				handler(ctx, msg, a.mqtt.Publish)
			}

		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

func (a *Actor) Interrupt(err error) {
	// nothing to do here yet
}
