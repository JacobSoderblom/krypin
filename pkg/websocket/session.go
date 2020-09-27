package websocket

import (
	"encoding/json"
	"fmt"
	"net/http"

	"gopkg.in/olahol/melody.v1"
)

type Session struct {
	RequestID string
	Topic     string
	Data      []byte
	Request   *http.Request

	msession *melody.Session
	mrouter  *melody.Melody
}

func newSession(msg []byte, s *melody.Session, r *melody.Melody) (*Session, error) {
	session := &Session{
		msession: s,
		mrouter:  r,
		Request:  s.Request,
	}

	var req Request
	if err := json.Unmarshal(msg, &req); err != nil {
		return session, NewError("bad_request", fmt.Sprintf("failed to parse request: %v", err))
	}

	session.RequestID = req.RequestID
	session.Topic = req.Topic

	b, err := json.Marshal(req.Data)
	if err != nil {
		return session, NewError("bad_request", fmt.Sprintf("failed to parse request data: %v", err))
	}

	session.Data = b

	return session, nil
}

func (s *Session) NoContent() error {
	return s.send(Response{
		RequestID: s.RequestID,
		Topic:     s.Topic,
	})
}

func (s *Session) JSON(data interface{}) error {
	return s.send(Response{
		RequestID: s.RequestID,
		Topic:     s.Topic,
		Result:    data,
	})
}

func (s *Session) Error(code string, err error) error {
	return s.send(Response{
		RequestID: s.RequestID,
		Topic:     s.Topic,
		Error:     NewError(code, err.Error()),
	})
}

func (s *Session) Close(code string, err error) error {
	b, err := s.parseResponse(Response{
		RequestID: s.RequestID,
		Topic:     s.Topic,
		Error:     NewError(code, err.Error()),
	})
	if err != nil {
		return err
	}

	if err = s.msession.CloseWithMsg(b); err != nil {
		return NewError("internal", fmt.Sprintf("failed to close session with message: %v", err))
	}

	return nil
}

func (s *Session) Broadcast(topic string, data interface{}) error {
	b, err := s.parseResponse(Response{
		RequestID: s.RequestID,
		Topic:     s.Topic,
		Result:    data,
	})
	if err != nil {
		return err
	}

	if err = s.mrouter.BroadcastOthers(b, s.msession); err != nil {
		return NewError("internal", fmt.Sprintf("failed to broadcast to other sessions: %v", err))
	}

	return nil
}

func (s *Session) Bind(dst interface{}) error {
	if err := json.Unmarshal(s.Data, dst); err != nil {
		return NewError("internal", fmt.Sprintf("could not parse request data: %v", err))
	}

	return nil
}

func (s *Session) send(resp Response) error {
	b, err := s.parseResponse(resp)
	if err != nil {
		return err
	}

	if err = s.msession.Write(b); err != nil {
		return NewError("internal", fmt.Sprintf("failed to write message to session: %v", err))
	}

	return nil
}

func (s *Session) parseResponse(resp Response) ([]byte, error) {
	b, err := json.Marshal(resp)
	if err != nil {
		return nil, NewError("internal", fmt.Sprintf("failed to parse response: %v", err))
	}

	return b, nil
}
