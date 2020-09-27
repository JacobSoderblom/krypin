package service

import (
	"context"
	"fmt"

	"github.com/JacobSoderblom/krypin/internal/errors"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/gofrs/uuid"
)

// Repository interface
type Repository interface {
	Insert(ctx context.Context, device *devicereg.Device) (*devicereg.Device, error)
	InsertEntityStates(ctx context.Context, states []*devicereg.EntityState) ([]*devicereg.EntityState, error)
	Select(ctx context.Context, deviceID uuid.UUID) (*devicereg.Device, error)
	SelectAll(ctx context.Context) ([]*devicereg.Device, error)
	SelectByIdentifier(ctx context.Context, identifier string) (*devicereg.Device, error)
	SelectLatestStateForEntities(ctx context.Context, IDs ...string) ([]*devicereg.EntityState, error)
}

// NewDeviceService creates a new service for devices
func NewDeviceService(db Repository) devicereg.Service {
	return &deviceService{
		db: db,
	}
}

type deviceService struct {
	db Repository
}

func (s deviceService) Add(ctx context.Context, d devicereg.Device) (*devicereg.Device, error) {
	device, err := s.db.SelectByIdentifier(ctx, d.Identifier)
	if err != nil && !errors.Is(errors.NotFound, err) {
		return nil, err
	}

	if device != nil {
		return nil, errors.New(errors.Conflict, fmt.Sprintf("device with identifier (%s) already exist", d.Identifier))
	}

	device, err = s.db.Insert(ctx, &d)
	if err != nil {
		return nil, err
	}

	return device, nil
}

func (s deviceService) Get(ctx context.Context, id uuid.UUID) (*devicereg.Device, error) {
	return s.db.Select(ctx, id)
}

func (s deviceService) List(ctx context.Context) ([]*devicereg.Device, error) {
	devices, err := s.db.SelectAll(ctx)
	if err != nil && !errors.Is(errors.NotFound, err) {
		return nil, err
	}
	return devices, nil
}
