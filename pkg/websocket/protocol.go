package websocket

import (
	"fmt"
)

type Request struct {
	RequestID string      `json:"requestId"`
	Topic     string      `json:"topic"`
	Data      interface{} `json:"data"`
}

type Response struct {
	RequestID string      `json:"requestId,omitempty"`
	Topic     string      `json:"topic"`
	Result    interface{} `json:"result,omitempty"`
	Error     *Error      `json:"errors,omitempty"`
}

type Error struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func NewError(code string, msg string) *Error {
	return &Error{
		Code:    code,
		Message: msg,
	}
}

func (e *Error) Error() string {
	return fmt.Sprintf("<%s> %s", e.Code, e.Message)
}
