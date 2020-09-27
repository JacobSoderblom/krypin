package service

import (
	"context"
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/internal/errors"
)

func (s deviceService) AddEntityStates(ctx context.Context, states ...*devicereg.EntityState) ([]*devicereg.EntityState, error) {
	entityIDs := make([]string, len(states))

	for _, state := range states {
		entityIDs = append(entityIDs, state.EntityID)
	}

	foundStates, err := s.db.SelectLatestStateForEntities(ctx, entityIDs...)
	if err != nil {
		return nil, err
	}

	validForAdd := []*devicereg.EntityState{}

	for _, state := range states {
		foundState := getStateByEntityID(foundStates, state.EntityID)

		if foundState == nil {
			validForAdd = append(validForAdd, state)
			continue
		}

		if state.IsEqual(foundState.Value) {
			continue
		}

		err = state.Merge(foundState.Value)
		if err != nil {
			fmt.Println(err)
			return nil, err
		}

		validForAdd = append(validForAdd, state)
	}

	if len(validForAdd) == 0 {
		return []*devicereg.EntityState{}, nil
	}

	res, err := s.db.InsertEntityStates(ctx, validForAdd)
	if err != nil {
		if errors.Is(errors.NotFound, err) {
			return nil, errors.New(errors.Invalid, "cannot add states to entities that does not exist")
		}

		return nil, err
	}

	return res, nil
}
