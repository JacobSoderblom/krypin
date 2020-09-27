package transport

import (
	"context"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/pkg/websocket"
)

type SocketHandlerSet struct {
	List websocket.HandlerFn
}

func SocketHandlers(svc devicereg.Service) SocketHandlerSet {
	return SocketHandlerSet{
		List: List(svc),
	}
}

func List(svc devicereg.Service) websocket.HandlerFn {
	return func(ctx context.Context, s *websocket.Session) error {
		devices, err := svc.List(ctx)
		if err != nil {
			return s.Error("internal", err)
		}

		return s.JSON(devices)
	}
}
