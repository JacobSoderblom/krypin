package devicereg

import (
	"context"

	"github.com/gofrs/uuid"
)

type Service interface {
	Add(ctx context.Context, d Device) (*Device, error)
	Get(ctx context.Context, id uuid.UUID) (*Device, error)
	AddEntityStates(ctx context.Context, states ...*EntityState) ([]*EntityState, error)
	List(ctx context.Context) ([]*Device, error)
}
