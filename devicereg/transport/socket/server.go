package socket

import (
	"github.com/JacobSoderblom/krypin/devicereg/transport"
	"github.com/JacobSoderblom/krypin/pkg/websocket"
)

func NewSocketHandler(handlers transport.SocketHandlerSet, wsrouter *websocket.Router) {
	wsrouter.Handle("devices/list", handlers.List)
}
