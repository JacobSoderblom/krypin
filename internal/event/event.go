package event

import (
	"encoding/json"
	"fmt"

	"github.com/pkg/errors"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/pkg/timestamp"
)

const timeFormat = "2006-01-02 15:04:05.000"
const DiscoveredTopic = "device/discovered"
const EntityStateTopic = "entity/state_update"

type Event struct {
	Topic     string              `json:"-"`
	Timestamp timestamp.Timestamp `json:"timestamp,omitempty"`
	Payload   []byte              `json:"payload,omitempty"`
}

func New(topic string) *Event {
	ts := timestamp.Now()
	return &Event{Topic: fmt.Sprintf("krypin/%s", topic), Timestamp: ts}
}

func (event *Event) Bytes() []byte {
	v, _ := json.Marshal(event)
	return v
}

func (event *Event) String() string {
	return string(event.Bytes())
}

func (event *Event) SetPayload(d interface{}) error {
	b, err := json.Marshal(d)
	if err != nil {
		return errors.Wrap(err, "failed to marshal payload")
	}

	event.Payload = b

	return nil
}

func (event *Event) GetPayload(d interface{}) error {
	err := json.Unmarshal(event.Payload, d)
	if err != nil {
		return errors.Wrap(err, "failed to unmarshal payload")
	}

	return nil
}

func Parse(payload []byte, topic string) *Event {
	var ev Event
	err := json.Unmarshal(payload, &ev)
	if err != nil {
		return nil
	}

	ev.Topic = topic

	return &ev
}

func NewDiscovered(d *devicereg.Device) *Event {
	ev := New(DiscoveredTopic)
	ev.SetPayload(d)

	return ev
}

func NewEntityStateUpdate(e []*devicereg.EntityState) *Event {
	ev := New(EntityStateTopic)
	ev.SetPayload(e)

	return ev
}
