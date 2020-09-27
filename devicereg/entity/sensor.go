package entity

import (
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg"
)

// Sensor describes the entity sensor of a device
type Sensor struct {
	Value             interface{} `json:"state"`
	UnitOfMeasurement string      `json:"unit_of_measurement"`
	Class             string      `json:"class"`
}

func NewSensor(name, module string) *devicereg.Entity {
	return devicereg.NewEntity(fmt.Sprintf("sensor.%s", devicereg.UniqueEntityID(name)), name, "sensor", module)
}

// Sensor classes
var (
	Battery        string = "battery"
	Humidity              = "humidity"
	Illuminance           = "illuminance"
	SignalStrength        = "signal_strength"
	Temperature           = "temperature"
	Power                 = "power"
	Pressure              = "pressure"
	Time                  = "timestamp"
)
