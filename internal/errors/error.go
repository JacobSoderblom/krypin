package errors

import (
	"bytes"
	"errors"
	"fmt"
)

// Code defines a code for an error
type Code string

// Different Error codes
const (
	Conflict = Code("conflict")
	Internal = Code("internal")
	Invalid  = Code("invalid")
	NotFound = Code("not_found")
)

// Error defines an error
type Error struct {
	Code    Code
	Op      string
	Message string
	Err     error
}

func (e Error) Error() string {
	var buf bytes.Buffer

	// Print the current operation in our stack, if any.
	if e.Op != "" {
		fmt.Fprintf(&buf, "%s: ", e.Op)
	}

	// If wrapping an error, print its Error() message.
	// Otherwise print the error code & message.
	if e.Err != nil {
		buf.WriteString(e.Err.Error())
	} else {
		if e.Code != "" {
			fmt.Fprintf(&buf, "<%s> ", e.Code)
		}
		buf.WriteString(e.Message)
	}
	return buf.String()
}

// New creates a new error with code and message
func New(code Code, msg string) error {
	return &Error{
		Code:    code,
		Message: msg,
	}
}

// Is checks if an error is of provided code
func Is(code Code, err error) bool {
	var e *Error

	if !errors.As(err, &e) {
		return false
	}

	if code == e.Code {
		return true
	}

	return Is(code, e.Err)
}

// As returns the error as an Error
func As(err error) (*Error, bool) {
	var e *Error
	return e, errors.As(err, &e)
}

// Op either appends operation to current Error
// or creates a new Error if the provided error
// is not of type Error and adds the operation to it
func Op(err error, op string) error {
	e, ok := As(err)
	if !ok {
		return &Error{
			Op:  op,
			Err: err,
		}
	}

	e.Op = op

	return e
}
