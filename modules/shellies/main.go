package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/JacobSoderblom/krypin/internal/core"
	"github.com/JacobSoderblom/krypin/internal/event"
	"github.com/JacobSoderblom/krypin/modules/shellies/parse"
	"github.com/JacobSoderblom/krypin/pkg/actor/mqttact"
	"github.com/JacobSoderblom/krypin/pkg/actor/ticker"
	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
)

func main() {
	os.Setenv("MQTT_BROKER", fmt.Sprintf("%s:%s", "192.168.20.40", "1883"))

	ctx := context.Background()

	logger := core.NewLogger("shelly")
	errLogger := core.NewErrorLogger("shelly")

	mqttClient := core.OpenMqtt()

	mqttAct := mqttact.New(mqttClient)

	mqttAct.Handle("shellies/+/announce", func(ctx context.Context, msg mqtt.Message, publisher mqtt.Publish) error {

		d, err := parse.ParseAnnounce(msg.Payload())
		if err != nil {
			errLogger.Log("error", err)
			return err
		}

		ev := event.NewDiscovered(d)

		t := publisher(ev.Topic, 0, false, ev.Bytes())

		return t.Error()
	})

	mqttAct.Handle("shellies/+/info", func(ctx context.Context, msg mqtt.Message, publisher mqtt.Publish) error {

		identifier := msg.Topic()[strings.Index(msg.Topic(), "/")+1 : strings.Index(msg.Topic(), "/info")]

		e, err := parse.ParseInfo(identifier, msg.Payload())
		if err != nil {
			errLogger.Log("error", err)
			return err
		}

		ev := event.NewEntityStateUpdate(e)

		return publisher(ev.Topic, 0, false, ev.Bytes()).Error()
	})

	tickAct := ticker.New(60*time.Second, func(ctx context.Context) {
		mqttClient.Publish("shellies/command", 0, false, "announce")
	})

	m := core.NewModule("shelly", errLogger, logger)

	m.Add(mqttAct.Execute, mqttAct.Interrupt)
	m.Add(tickAct.Execute, tickAct.Interrupt)

	go func() {
		time.Sleep(100)

		mqttClient.Publish("shellies/command", 0, false, "announce")
	}()

	m.Run(ctx)
}
