package websocket

import "context"

type Log interface {
	Log(keyvals ...interface{}) error
}

func Logger(logger Log) Middleware {
	return func(next HandlerFn) HandlerFn {
		return func(ctx context.Context, s *Session) error {
			logger.Log("request_id", s.RequestID, "topic", s.Topic)

			return next(ctx, s)
		}
	}
}
