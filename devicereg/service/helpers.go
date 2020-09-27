package service

import (
	"github.com/JacobSoderblom/krypin/devicereg"
)

func getStateByEntityID(states []*devicereg.EntityState, entityID string) *devicereg.EntityState {
	for _, s := range states {
		if entityID != s.EntityID {
			continue
		}

		return s
	}

	return nil
}
