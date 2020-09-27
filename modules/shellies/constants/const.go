package constants

import (
	"strings"
)

// constants for Shellies
var (
	Shelly1ID   = "SHSW-1"
	Shelly1Name = "Shelly1"

	Shelly1PMID   = "SHSW-PM"
	Shelly1PMName = "Shelly1pm"

	Shelly2ID   = "SHSW-21"
	Shelly2Name = "ShellySwitch"

	Shelly25ID   = "SHSW-25"
	Shelly25Name = "ShellySwitch25"

	Shelly3EMID   = "SHEM-3"
	Shelly3EMName = "Shellyem3"

	Shelly4ProID   = "SHSW-44"
	Shelly4ProName = "Shelly4pro"

	ShellyAirID   = "SHAIR-1"
	ShellyAirName = "Shellyair"

	ShellyBulbID   = "SHBLB-1"
	ShellyBulbName = "Shellybulb"

	ShellyButton1ID   = "SHBTN-1"
	ShellyButton1Name = "Shellybutton1"

	ShellyDimmerID     = "SHDM-1"
	ShellyDimmerName   = "Shelly Dimmer"
	ShellyDimmerPrefix = "shellydimmer"

	ShellyDimmer2ID     = "SHDM-2"
	ShellyDimmer2Name   = "Shelly Dimmer 2"
	ShellyDimmer2Prefix = "shellydimmer2"

	ShellyDuoID   = "SHBDUO-1"
	ShellyDuoName = "Shellybulbduo"
)

func RemovePrefix(id string) string {
	return id[strings.Index(id, "-")+1:]
}
