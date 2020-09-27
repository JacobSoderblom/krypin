package core

import (
	"os"

	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
)

func OpenMqtt() *mqtt.Client {
	opts := mqtt.NewClientOptions(os.Getenv("MQTT_BROKER"), "krypin")

	client, err := mqtt.New(opts)
	if err != nil {
		panic(err)
	}

	return client
}
