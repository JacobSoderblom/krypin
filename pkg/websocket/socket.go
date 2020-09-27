package websocket

import (
	"context"
	"fmt"
	"sync"

	"gopkg.in/olahol/melody.v1"
)

type Broadcaster interface {
	Broadcast(msg []byte) error
}
type HandlerFn func(context.Context, *Session) error
type Middleware func(next HandlerFn) HandlerFn

type Router struct {
	middlewares        []Middleware
	handlerMap         map[string]HandlerFn
	connectHandlers    []HandlerFn
	disconnectHandlers []HandlerFn
	mrouter            *melody.Melody

	lock sync.Mutex
}

func NewRouter(m *melody.Melody) *Router {
	return &Router{
		middlewares:        []Middleware{},
		handlerMap:         map[string]HandlerFn{},
		connectHandlers:    []HandlerFn{},
		disconnectHandlers: []HandlerFn{},
		mrouter:            m,
	}
}

func (r *Router) Handler(s *melody.Session, msg []byte) {
	r.lock.Lock()
	defer r.lock.Unlock()

	session, err := newSession(msg, s, r.mrouter)
	if err != nil {
		session.Error("internal", err)
		return
	}

	handler, exist := r.handlerMap[session.Topic]
	if !exist {
		return
	}

	for _, mw := range reverse(r.middlewares) {
		handler = mw(handler)
	}

	err = handler(s.Request.Context(), session)
	if err == nil {
		return
	}

	if e, ok := err.(*Error); ok {
		session.Error(e.Code, e)
		return
	}

	s.Write([]byte(err.Error()))
}

func (r *Router) HandleConnect(s *melody.Session) {
	r.lock.Lock()
	defer r.lock.Unlock()

	fmt.Println("connect")

	session := Session{
		msession: s,
		mrouter:  r.mrouter,
		Request:  s.Request,
	}

	var err error

	for _, c := range r.connectHandlers {
		if err = c(s.Request.Context(), &session); err != nil {
			break
		}
	}

	if err == nil {
		return
	}

	if e, ok := err.(*Error); ok {
		session.Error(e.Code, e)
		return
	}

	s.Write([]byte(err.Error()))
}

func (r *Router) HandleDisconnect(s *melody.Session) {
	r.lock.Lock()
	defer r.lock.Unlock()

	session := Session{
		msession: s,
		mrouter:  r.mrouter,
		Request:  s.Request,
	}

	var err error

	for _, c := range r.disconnectHandlers {
		if err = c(s.Request.Context(), &session); err != nil {
			break
		}
	}

	if err == nil {
		return
	}

	if e, ok := err.(*Error); ok {
		session.Error(e.Code, e)
		return
	}

	s.Write([]byte(err.Error()))
}

func (r *Router) Handle(topic string, handler HandlerFn, middlewares ...Middleware) {
	for _, mw := range reverse(middlewares) {
		handler = mw(handler)
	}

	r.handlerMap[topic] = handler
}

func (r *Router) onConnnect(handler HandlerFn) {
	r.connectHandlers = append(r.connectHandlers, handler)
}

func (r *Router) onDisconnnect(handler HandlerFn) {

	r.disconnectHandlers = append(r.disconnectHandlers, handler)
}

func (r *Router) Use(mw ...Middleware) {
	r.middlewares = append(r.middlewares, mw...)
}

func reverse(middlewares []Middleware) []Middleware {
	for i, j := 0, len(middlewares)-1; i < j; i, j = i+1, j-1 {
		middlewares[i], middlewares[j] = middlewares[j], middlewares[i]
	}
	return middlewares
}
