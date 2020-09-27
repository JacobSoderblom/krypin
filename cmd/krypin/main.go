package main

import (
	"context"
	"os"

	"github.com/JacobSoderblom/krypin/internal/core"
	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/JacobSoderblom/krypin/pkg/log"
	"github.com/JacobSoderblom/krypin/pkg/protocol/mqtt"
	"github.com/JacobSoderblom/krypin/pkg/websocket"
	"github.com/labstack/echo/v4"
	"gopkg.in/olahol/melody.v1"
)

var (
	db         *database.Database
	mqttclient *mqtt.Client

	logger    log.Logger
	errlogger log.Logger

	module *core.Module

	apigroup *echo.Group
	wsrouter *websocket.Router
	socket   websocket.Broadcaster
)

func main() {

	os.Setenv("DATABASE_URL", "postgres://postgres:password@localhost:5432/krypin?sslmode=disable")

	ctx := context.Background()

	logger = core.NewLogger("core")
	errlogger = core.NewErrorLogger("core")

	mqttclient = core.OpenMqtt()

	var err error
	db, err = database.Connect(ctx)
	if err != nil {
		errlogger.Log("error", err)
		panic(err)
	}

	module = core.NewModule("core", errlogger, logger)

	e := echo.New()
	m := melody.New()
	socket = m
	wsrouter = websocket.NewRouter(m)

	wsrouter.Use(websocket.Logger(logger), database.SocketTransaction(db))

	e.GET("/ws", func(c echo.Context) error {
		m.HandleRequest(c.Response(), c.Request())
		return nil
	})

	apigroup = e.Group("/api")

	setupDevicereg()

	m.HandleMessage(wsrouter.Handler)
	m.HandleConnect(wsrouter.HandleConnect)
	m.HandleDisconnect(wsrouter.HandleDisconnect)

	module.Add(func(ctx context.Context) error {
		return e.Start(":3000")
	}, func(err error) {
		e.Close()
	})

	module.Run(ctx)
}
