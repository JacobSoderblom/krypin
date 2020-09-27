package entity

import (
	"fmt"
	"regexp"
	"strconv"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/lucasb-eyer/go-colorful"
)

// Light describes the entity for light and bulbs
type Light struct {
	Brightness int        `json:"brightness"`
	HSL        string     `json:"hsl"`
	IsOn       bool       `json:"is_on"`
	ColorTemp  *ColorTemp `json:"color_temp"`
	Mode       string     `json:"mode"`
}

// NewLight creates a new light entity
func NewLight(name, module string, features ...string) *devicereg.Entity {
	uniqueID := devicereg.UniqueEntityID(name)

	e := devicereg.NewEntity(fmt.Sprintf("light.%s", uniqueID), name, "light", module)
	e.Features = features

	return e
}

// ColorTemp color value in mireds
type ColorTemp struct {
	Value int `json:"value,omitempty"`
	Max   int `json:"max,omitempty"`
	Min   int `json:"min,omitempty"`
}

// RGBToHSLString converts the R,G,B values to a HSL string
func RGBToHSLString(r, g, b int) string {
	c := colorful.Color{
		R: float64(r) / 360,
		G: float64(g) / 255,
		B: float64(b) / 255,
	}
	h, s, l := c.Hsl()
	return HSLConstruct(int32(h), int32(s*100), int32(l*100))
}

// HSLStringToRGB converts a HSL string e.g. hsl(100, 32%, 99%) to its corresponding RGB values
func HSLStringToRGB(hsl string) (byte, byte, byte, error) {
	h, s, l, err := HSLDeconstruct(hsl)
	if err != nil {
		return 0, 0, 0, err
	}

	c := colorful.Hsl(float64(h), float64(s)/100, float64(l)/100)
	return byte(c.R * 255), byte(c.G * 255), byte(c.B * 255), nil
}

var hslRegexp = regexp.MustCompile("hsl\\((.+)\\s*,\\s*(.+)%\\s*,\\s*(.+)%\\s*\\)")

// HSLDeconstruct takes in a HSL string e.g. hsl(255, 44%, 32%) and returns the HSL
// components e.g. 255,44,32
func HSLDeconstruct(val string) (int32, int32, int32, error) {
	//format hsl(100, 50%, 50%)
	matches := hslRegexp.FindStringSubmatch(val)
	if len(matches) == 0 {
		return 0, 0, 0, fmt.Errorf("invalid HSL format")
	}

	hue, err := strconv.Atoi(matches[1])
	if err != nil {
		return 0, 0, 0, fmt.Errorf("invalid hue value, must be an integer")
	}

	saturation, err := strconv.Atoi(matches[2])
	if err != nil {
		return 0, 0, 0, fmt.Errorf("invalid saturation value, must be an integer")
	}

	luminence, err := strconv.Atoi(matches[3])
	if err != nil {
		return 0, 0, 0, fmt.Errorf("invalid luminence value, must be an integer")
	}

	return int32(hue), int32(saturation), int32(luminence), nil
}

// HSLConstruct takes in H,S,L values and converts then to a HSL string e.g. hsl(255, 44%, 32%)
func HSLConstruct(hue, saturation, luminence int32) string {
	return fmt.Sprintf("hsl(%d, %d%%, %d%%)", hue, saturation, luminence)
}
