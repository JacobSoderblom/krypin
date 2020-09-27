package devicereg

import (
	"errors"
	"reflect"
	"strings"

	"github.com/JacobSoderblom/krypin/pkg/timestamp"
	"github.com/gofrs/uuid"
	"github.com/imdario/mergo"
)

// Domain errors for devicereg
var (
	ErrAlreadyExist = errors.New("device already exist")
	ErrNotFound     = errors.New("device not found")
)

// Device is a piece of hardware. It could be a dimmer, a shade, a remote etc
type Device struct {
	ID              uuid.UUID           `json:"id,omitempty"`
	Name            string              `json:"name,omitempty"`
	Manufacturer    string              `json:"manufacturer,omitempty"`
	Model           string              `json:"model,omitempty"`
	SoftwareVersion string              `json:"sw_version,omitempty"`
	Identifier      string              `json:"identifier,omitempty"`
	Connections     []*Connection       `json:"connections,omitempty"`
	Entities        []*Entity           `json:"entites,omitempty"`
	CreatedAt       timestamp.Timestamp `json:"created_at,omitempty"`
	UpdatedAt       timestamp.Timestamp `json:"updated_at,omitempty"`
}

// New creates a new device
func New(identifier, manufacturer, model, swVer string) *Device {
	return &Device{
		Name:            identifier,
		Identifier:      identifier,
		Manufacturer:    manufacturer,
		Model:           model,
		SoftwareVersion: swVer,
	}
}

// AddConnection adds a new connection to the device
func (d *Device) AddConnection(connectionType, Value string) {
	d.Connections = append(d.Connections, &Connection{
		Type:  connectionType,
		Value: Value,
	})
}

// AddEntity adds a new entity to the device
func (d *Device) AddEntity(e Entity) {
	d.Entities = append(d.Entities, &e)
}

// AddEntities adds entities to the device
func (d *Device) AddEntities(entities ...*Entity) {
	d.Entities = append(d.Entities, entities...)
}

// Connection describes a device connection
type Connection struct {
	Type  string `json:"type,omitempty"`
	Value string `json:"value,omitempty"`
}

// Entity Entity
type Entity struct {
	ID        string              `json:"id"`
	Name      string              `json:"name"`
	Type      string              `json:"type"`
	States    []*EntityState      `json:"state"`
	Module    string              `json:"module"`
	Features  []string            `json:"features,omitempty"`
	DeviceID  uuid.UUID           `json:"device_id,omitempty"`
	CreatedAt timestamp.Timestamp `json:"created_at,omitempty"`
	UpdatedAt timestamp.Timestamp `json:"updated_at,omitempty"`
}

func NewEntity(id, name, entityType, module string) *Entity {
	return &Entity{
		ID:     id,
		Name:   name,
		Type:   entityType,
		Module: module,
	}
}

func (e *Entity) AddState(state interface{}) {
	e.States = append(e.States, &EntityState{
		Value:    state,
		EntityID: e.ID,
	})
}

func (e *Entity) AddFeatures(f ...string) {
	e.Features = append(e.Features, f...)
}

type EntityState struct {
	Value     interface{}          `json:"value"`
	CreatedAt *timestamp.Timestamp `json:"created_at,omitempty"`
	EntityID  string               `json:"entity_id,omitempty"`
}

func (s *EntityState) IsEqual(val interface{}) bool {
	if val == nil {
		return false
	}

	if s.Value == nil {
		return false
	}

	return reflect.DeepEqual(val, s.Value)
}

func (s *EntityState) Merge(val interface{}) error {
	if val == nil {
		return nil
	}

	if s.Value == nil {
		s.Value = val
		return nil
	}

	mapval := val.(map[string]interface{})
	dst := s.Value.(map[string]interface{})

	if err := mergo.Merge(&dst, mapval); err != nil {
		return err
	}

	s.Value = dst

	return nil
}

func UniqueEntityID(id string) string {
	uniqueID := strings.ReplaceAll(id, "-", "_")
	uniqueID = strings.ReplaceAll(uniqueID, " ", "_")
	return strings.ToLower(uniqueID)

}
