package entity

import (
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg"
)

// Switch describes the entity switch
type Switch struct {
	IsOn bool `json:"is_on"`
}

func NewSwitch(name, module string) *devicereg.Entity {
	return devicereg.NewEntity(fmt.Sprintf("binary_switch.%s", devicereg.UniqueEntityID(name)), name, "binary_switch", module)
}
