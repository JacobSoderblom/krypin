package parse

import (
	"encoding/json"
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/devicereg/entity"
	"github.com/JacobSoderblom/krypin/modules/shellies/constants"

	"github.com/pkg/errors"
)

type announce struct {
	ID              string `json:"id"`
	Model           string `json:"model"`
	Mac             string `json:"mac"`
	IP              string `json:"ip"`
	SoftwareVersion string `json:"fw_ver"`
}

func ParseAnnounce(b []byte) (*devicereg.Device, error) {
	a := &announce{}

	err := json.Unmarshal(b, a)
	if err != nil {
		return nil, errors.Wrap(err, "Failed to parse announce payload")
	}

	d := devicereg.New(a.ID, "Shelly", getModelName(a.Model), a.SoftwareVersion)

	d.AddConnection("network_mac", a.Mac)
	d.AddConnection("network_ip", a.IP)
	d.AddConnection("mqtt", "shellies")

	d.AddEntities(getEntities(d, a.Model)...)

	return d, nil
}

func getModelName(model string) string {
	switch model {
	case constants.ShellyDimmerID:
		return constants.ShellyDimmerName
	case constants.ShellyDimmer2ID:
		return constants.ShellyDimmer2Name
	default:
		return ""
	}
}

func getEntities(d *devicereg.Device, model string) []*devicereg.Entity {
	switch model {
	case constants.ShellyDimmer2ID:
		l := entity.NewLight(fmt.Sprintf("%s Light 0", d.Identifier), "shelly", entity.Brightness)
		s := entity.NewSensor(fmt.Sprintf("%s Energy", d.Identifier), "shelly")
		overpower := entity.NewSwitch(fmt.Sprintf("%s Over Power", d.Identifier), "shelly")
		overheating := entity.NewSwitch(fmt.Sprintf("%s Overheating", d.Identifier), "shelly")

		return []*devicereg.Entity{
			l,
			s,
			overpower,
			overheating,
		}
	case constants.ShellyDimmerID:
		l := entity.NewLight(fmt.Sprintf("%s Light 0", d.Identifier), "shelly", entity.Brightness)
		s := entity.NewSensor(fmt.Sprintf("%s Energy", d.Identifier), "shelly")
		overpower := entity.NewSwitch(fmt.Sprintf("%s Over Power", d.Identifier), "shelly")
		overheating := entity.NewSwitch(fmt.Sprintf("%s Overheating", d.Identifier), "shelly")

		return []*devicereg.Entity{
			l,
			s,
			overpower,
			overheating,
		}
	}

	return []*devicereg.Entity{}
}
