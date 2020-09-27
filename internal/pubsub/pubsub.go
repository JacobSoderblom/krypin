package pubsub

import (
	"context"
	"fmt"
	"regexp"
	"strings"

	"github.com/JacobSoderblom/krypin/internal/event"
	"github.com/JacobSoderblom/krypin/pkg/actor/mqttact"
	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
)

type Publisher interface {
	Publish(*event.Event) error
}

type Publish func(*event.Event) error
type Handler func(context.Context, *event.Event, Publish) error
type Middleware func(next Handler) Handler

func New(client *mqtt.Client) *PubSub {
	return &PubSub{
		mqtt:     client,
		act:      mqttact.New(client),
		handlers: map[string]Handler{},
	}
}

type PubSub struct {
	handlers map[string]Handler
	mqtt     *mqtt.Client
	act      *mqttact.Actor
}

func (p *PubSub) Handle(topic string, fn Handler, middlwares ...Middleware) {
	handler := fn

	for _, mw := range middlwares {
		handler = mw(handler)
	}

	p.handlers[fmt.Sprintf("krypin/%s", topic)] = handler
}

func (p *PubSub) Publish(ev *event.Event) error {
	t := p.mqtt.Publish(ev.Topic, 0, false, ev)

	return t.Error()
}

func (p *PubSub) handleMqttMessage(ctx context.Context, msg mqtt.Message, publisher mqtt.Publish) error {
	ev := event.Parse(msg.Payload(), msg.Topic())

	for topic, handler := range p.handlers {
		foundOneLevel, _ := regexp.MatchString(strings.ReplaceAll(topic, "+", "([a-zA-Z0-9 _.-]+)"), msg.Topic())

		if !foundOneLevel {
			continue
		}

		handler(ctx, ev, p.Publish)
	}

	return nil
}

func (p *PubSub) Execute(ctx context.Context) error {
	for topic, _ := range p.handlers {
		p.act.Handle(topic, p.handleMqttMessage)
	}

	return p.act.Execute(ctx)
}

func (p *PubSub) Interrupt(err error) {
	p.act.Interrupt(err)
}
