package database

import (
	"context"

	"github.com/JacobSoderblom/krypin/internal/event"
	"github.com/JacobSoderblom/krypin/internal/pubsub"
	"github.com/JacobSoderblom/krypin/pkg/websocket"

	"github.com/labstack/echo/v4"
)

// HttpTransaction creates a database transaction and appends it to the context
func HttpTransaction(db *Database) echo.MiddlewareFunc {
	return func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c echo.Context) error {
			return WithTransactionContext(c.Request().Context(), db, func(ctx context.Context) error {
				c.SetRequest(c.Request().WithContext(ctx))

				return next(c)
			})
		}
	}
}

func PubSubTransaction(db *Database) pubsub.Middleware {
	return func(next pubsub.Handler) pubsub.Handler {
		return func(ctx context.Context, ev *event.Event, publisher pubsub.Publish) error {
			return WithTransactionContext(ctx, db, func(ctxWithTransaction context.Context) error {
				return next(ctxWithTransaction, ev, publisher)
			})
		}
	}
}

func SocketTransaction(db *Database) websocket.Middleware {
	return func(next websocket.HandlerFn) websocket.HandlerFn {
		return func(ctx context.Context, s *websocket.Session) error {
			return WithTransactionContext(ctx, db, func(ctxWithTransaction context.Context) error {
				return next(ctxWithTransaction, s)
			})
		}
	}
}
