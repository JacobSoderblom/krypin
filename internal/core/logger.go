package core

import (
	"github.com/JacobSoderblom/krypin/pkg/log"
	"github.com/sirupsen/logrus"
)

func NewLogger(name string) log.Logger {
	l := logrus.WithField("module", name)

	return log.NewLogger(l)
}

func NewErrorLogger(name string) log.Logger {
	l := logrus.WithField("module", name)

	return log.NewLogger(l, log.WithLevel(logrus.ErrorLevel))
}
