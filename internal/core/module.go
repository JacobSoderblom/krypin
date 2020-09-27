package core

import (
	"context"
	"os"
	"syscall"

	"github.com/JacobSoderblom/krypin/pkg/actor"
	"github.com/JacobSoderblom/krypin/pkg/log"
)

func NewModule(name string, errLogger log.Logger, infoLogger log.Logger) *Module {
	return &Module{
		Name:       name,
		errLogger:  errLogger,
		infoLogger: infoLogger,
	}
}

type Module struct {
	Name       string
	g          actor.Group
	errLogger  log.Logger
	infoLogger log.Logger
}

func (m *Module) Add(execute actor.Execute, interrupt actor.Interrupt) {
	m.g.Add(execute, interrupt)
}

func (m *Module) Run(ctx context.Context) {
	m.g.Add(actor.SignalHandler(os.Interrupt, syscall.SIGINT, syscall.SIGTERM))

	m.infoLogger.Log("status", "starting")

	if err := m.g.Run(ctx); err != nil {
		m.errLogger.Log("error", err)
	}

	m.infoLogger.Log("status", "exiting")
}
