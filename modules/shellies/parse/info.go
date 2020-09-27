package parse

import (
	"encoding/json"
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg/entity"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/pkg/errors"
)

type info struct {
	Lights []light `json:"lights"`
	Meters []meter `json:"meters"`

	OverTemperature bool `json:"overtemperature"`
	Overload        bool `json:"overload"`
}

type light struct {
	IsOn       bool   `json:"ison"`
	Mode       string `json:"mode"`
	Brightness int    `json:"brightness"`
}

type meter struct {
	Power float32 `json:"power"`
	Total float32 `json:"total"`
}

func ParseInfo(identifier string, payload []byte) ([]*devicereg.EntityState, error) {
	i := &info{}

	err := json.Unmarshal(payload, i)
	if err != nil {
		return nil, errors.Wrap(err, "Failed to parse info payload")
	}

	states := []*devicereg.EntityState{}

	for i, l := range i.Lights {
		e := entity.NewLight(fmt.Sprintf("%s Light %v", identifier, i), "shelly")

		e.AddState(entity.Light{
			IsOn:       l.IsOn,
			Brightness: l.Brightness,
			Mode:       l.Mode,
		})

		states = append(states, e.States...)
	}

	for index, m := range i.Meters {
		name := fmt.Sprintf("%s Energy", identifier)

		if len(i.Meters) > 1 {
			name = fmt.Sprintf("%s Energy %v", identifier, index)
		}

		s := entity.NewSensor(name, "shelly")

		s.AddState(entity.Sensor{
			UnitOfMeasurement: entity.Watt,
			Class:             entity.Power,
			Value:             m.Power,
		})

		states = append(states, s.States...)
	}

	overpower := entity.NewSwitch(fmt.Sprintf("%s Over Power", identifier), "shelly")
	overpower.AddState(entity.Switch{
		IsOn: i.Overload,
	})

	overheating := entity.NewSwitch(fmt.Sprintf("%s Overheating", identifier), "shelly")
	overheating.AddState(entity.Switch{
		IsOn: i.OverTemperature,
	})

	states = append(states, overpower.States...)
	states = append(states, overheating.States...)

	return states, nil
}
