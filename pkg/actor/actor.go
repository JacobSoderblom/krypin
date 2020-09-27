// Heavily inspered by https://github.com/oklog/run

package actor

import (
	"context"
	"fmt"
	"os"
	"os/signal"
)

// Execute represents the execute func
type Execute func(context.Context) error

// Interrupt represents the interrupt func
type Interrupt func(error)

// Group collects actors
type Group struct {
	actors []actor
}

// Add an actor to the group.
// If the actor is a long running it should listen
// to context.Done() channel for exiting
func (g *Group) Add(execute Execute, interrupt Interrupt) {
	g.actors = append(g.actors, actor{execute, interrupt})
}

// Run all actors.
// Creating a new context with cancel function based on the provided context.
// When one actor returns, all other will be interrupted
// and Run will wait until all actors have exited.
func (g *Group) Run(ctx context.Context) error {
	if len(g.actors) == 0 {
		return nil
	}

	// Run each actor.
	errors := make(chan error, len(g.actors))
	cctx, cancel := context.WithCancel(ctx)
	for _, a := range g.actors {
		go func(a actor) {
			errors <- a.execute(cctx)
		}(a)
	}

	// Wait for the first actor to stop.
	err := <-errors

	cancel()

	// Signal all actors to stop.
	for _, a := range g.actors {
		a.interrupt(err)
	}

	// Wait for all actors to stop.
	for i := 1; i < cap(errors); i++ {
		<-errors
	}

	// Return the original error.
	return err
}

type actor struct {
	execute   Execute
	interrupt Interrupt
}

// SignalHandler returns an actor, listening for os signals
func SignalHandler(signals ...os.Signal) (Execute, Interrupt) {
	return func(ctx context.Context) error {
		c := make(chan os.Signal, 1)
		signal.Notify(c, signals...)
		defer signal.Stop(c)
		select {
		case sig := <-c:
			return SignalError{Signal: sig}
		case <-ctx.Done():
			return ctx.Err()
		}
	}, func(error) {}
}

// SignalError is returned by the signal handler's execute function
// when it terminates due to a received signal.
type SignalError struct {
	Signal os.Signal
}

// Error implements the error interface.
func (e SignalError) Error() string {
	return fmt.Sprintf("received signal %s", e.Signal)
}
