package event

import (
	"fmt"
)

// CommandTopic returns a command topic with provided string
func CommandTopic(topic string) string {
	return fmt.Sprintf("command/%s", topic)
}

// DiscoverTopic returns a discover command topic with provided string
func DiscoverTopic(topic string) string {
	return fmt.Sprintf("%s/discover", topic)
}

// NewCommand creates a new event with command topic
func NewCommand(topic string) *Event {
	ev := New(CommandTopic(topic))

	return ev
}

// NewDiscover creates a new event with discover commad topic
func NewDiscover(topic string) *Event {
	ev := New(DiscoverTopic(topic))

	return ev
}
