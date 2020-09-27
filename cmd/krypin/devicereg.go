package main

import (
	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/devicereg/repository"
	"github.com/JacobSoderblom/krypin/devicereg/service"
	"github.com/JacobSoderblom/krypin/devicereg/transport"
	http_transport "github.com/JacobSoderblom/krypin/devicereg/transport/http"
	pubsub_transport "github.com/JacobSoderblom/krypin/devicereg/transport/pubsub"
	socket_transport "github.com/JacobSoderblom/krypin/devicereg/transport/socket"
)

var (
	devicerep    service.Repository
	deviceregsvc devicereg.Service
)

func setupDevicereg() {
	devicerep := repository.NewDevice(db)
	deviceregsvc := service.NewDeviceService(devicerep)

	pubsubHandlers := transport.PubSubHandlers(deviceregsvc, socket)
	module.Add(pubsub_transport.NewPubSubActor(pubsubHandlers, mqttclient, db, errlogger))

	deviceEndpoints := transport.Endpoints(deviceregsvc)
	apigroup = http_transport.NewHttpHandler(deviceEndpoints, apigroup, db)

	socketHandlers := transport.SocketHandlers(deviceregsvc)
	socket_transport.NewSocketHandler(socketHandlers, wsrouter)
}
