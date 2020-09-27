package mqtt

import (
	"fmt"
	"math/rand"
	"os"
	"sync"

	MQTT "github.com/eclipse/paho.mqtt.golang"
	"github.com/pkg/errors"
)

type Handler func(*Client, *Message)

type Handlers []Handler

type Publish func(topic string, qos byte, retained bool, payload interface{}) MQTT.Token

type Message struct {
	MQTT.Message
}

type ClientOptions struct {
	MQTT.ClientOptions
}

type Client struct {
	MQTT.Client
	onDefaultPublishHandlers Handlers
	onConnectionHandlers     Handlers
	m                        sync.Mutex
}

func New(opts *MQTT.ClientOptions) (*Client, error) {
	c := &Client{
		onConnectionHandlers:     Handlers{},
		onDefaultPublishHandlers: Handlers{},
		m:                        sync.Mutex{},
	}

	opts.SetDefaultPublishHandler(c.defaultPublishHandler)
	opts.SetOnConnectHandler(c.onConnectionHandler)

	client := MQTT.NewClient(opts)
	if token := client.Connect(); token.Wait() && token.Error() != nil {
		return nil, errors.Wrap(token.Error(), "Failed to connect to mqtt broker")
	}

	c.Client = client

	return c, nil
}

func (c *Client) AddDefaultPublishHandler(h Handler) {
	c.m.Lock()
	defer c.m.Unlock()

	c.onDefaultPublishHandlers = append(c.onDefaultPublishHandlers, h)
}

func (c *Client) AddConnectionHandler(h Handler) {
	c.m.Lock()
	defer c.m.Unlock()

	c.onConnectionHandlers = append(c.onConnectionHandlers, h)
}

func (c *Client) Subscribe(topic string, qos byte) <-chan Message {
	messageChannel := make(chan Message)

	c.Client.Subscribe(topic, qos, func(client MQTT.Client, msg MQTT.Message) {
		messageChannel <- Message{Message: msg}
	})

	return messageChannel
}

func (c *Client) SubscribeFn(topic string, qos byte, fn func(msg Message)) {
	c.Client.Subscribe(topic, qos, func(client MQTT.Client, msg MQTT.Message) {
		fn(Message{Message: msg})
	})
}

func (c *Client) SubscribeMultiple(filter map[string]byte) <-chan Message {
	messageChannel := make(chan Message)

	c.Client.SubscribeMultiple(filter, func(client MQTT.Client, msg MQTT.Message) {
		messageChannel <- Message{Message: msg}
	})

	return messageChannel
}

func (c *Client) defaultPublishHandler(client MQTT.Client, msg MQTT.Message) {
	c.m.Lock()
	defer c.m.Unlock()

	m := &Message{
		Message: msg,
	}

	for _, h := range c.onDefaultPublishHandlers {
		h(c, m)
	}
}

func (c *Client) onConnectionHandler(Client MQTT.Client) {
	c.m.Lock()
	defer c.m.Unlock()

	for _, h := range c.onConnectionHandlers {
		h(c, nil)
	}
}

func NewClientOptions(broker, prefix string) *MQTT.ClientOptions {
	hostname, _ := os.Hostname()
	pid := os.Getpid()
	r := rand.Int()
	clientID := fmt.Sprintf("%s/%s-%d-%d", prefix, hostname, pid, r)
	opts := MQTT.NewClientOptions()
	opts.AddBroker(broker)
	opts.SetClientID(clientID)
	opts.SetCleanSession(true)
	return opts
}
