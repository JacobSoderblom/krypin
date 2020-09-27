package repository

import (
	"context"
	"fmt"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/devicereg/service"
	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/Masterminds/squirrel"
	"github.com/gofrs/uuid"
	"github.com/randallmlough/pgxscan"
)

var sq = squirrel.StatementBuilder.PlaceholderFormat(squirrel.Dollar)

var EmptyID = uuid.UUID{}

type deviceRepository struct {
	db *database.Database
}

func NewDevice(db *database.Database) service.Repository {
	return &deviceRepository{
		db: db,
	}
}

func (r *deviceRepository) Insert(ctx context.Context, d *devicereg.Device) (*devicereg.Device, error) {

	model := toDeviceModel(d)

	sql, args, err := sq.
		Insert("devices").Columns("name", "manufacturer", "model", "sw_ver", "identifier").
		Values(model.Name, model.Manufacturer, model.Model, model.SoftwareVersion, model.Identifier).
		Suffix("RETURNING id, created_at, updated_at").
		ToSql()
	if err != nil {
		return nil, database.GetError(fmt.Errorf("could not create sql for insert of device: %w", err))
	}

	tx := r.db.Tx(ctx)

	if err := tx.QueryRow(ctx, sql, args...).Scan(&model.ID, &model.CreatedAt, &model.UpdatedAt); err != nil {
		return nil, database.GetError(fmt.Errorf("failed to insert device: %w", err))
	}

	if len(model.Entities) > 0 {
		entities, err := r.insertEntities(ctx, model.ID, model.Entities)
		if err != nil {
			return nil, err
		}

		model.Entities = entities
	}

	return fromDeviceModel(model), nil
}

func (r *deviceRepository) InsertEntityStates(ctx context.Context, states []*devicereg.EntityState) ([]*devicereg.EntityState, error) {
	q := sq.
		Insert("entity_states").Columns("value", "entity_id")

	for _, s := range states {
		m := toEntityStateModel(s)
		q = q.Values(m.Value, m.EntityID)
	}

	sql, args, err := q.
		Suffix("RETURNING created_at, entity_id, value").
		ToSql()
	if err != nil {
		return nil, database.GetError(fmt.Errorf("could not create sql for insert of entity state: %w", err))
	}

	tx := r.db.Tx(ctx)

	rows, err := tx.Query(ctx, sql, args...)
	if err != nil {
		return nil, database.GetError(fmt.Errorf("failed to insert entity state: %w", err))
	}

	var models []*entityState

	if err = pgxscan.NewScanner(rows).Scan(&models); err != nil {
		return nil, database.GetError(fmt.Errorf("could not scan entity states into struct: %w", err))
	}

	states = []*devicereg.EntityState{}

	for _, m := range models {
		states = append(states, fromEntityStateModel(m))
	}

	return states, nil
}

func (r *deviceRepository) insertEntities(ctx context.Context, deviceID uuid.UUID, entities []*deviceEntity) ([]*deviceEntity, error) {
	q := sq.
		Insert("device_entities").Columns("id", "features", "type", "device_id", "name", "module")

	for _, e := range entities {
		q = q.Values(e.ID, e.Features, e.Type, deviceID, e.Name, e.Module)
	}

	sql, args, err := q.
		Suffix("RETURNING *").
		ToSql()
	if err != nil {
		return nil, database.GetError(fmt.Errorf("could not create sql for insert of device entities: %w", err))
	}

	tx := r.db.Tx(ctx)

	var model []*deviceEntity

	entityRows, err := tx.Query(ctx, sql, args...)
	if err != nil {
		return nil, database.GetError(fmt.Errorf("failed to insert device entities: %w", err))
	}

	if err = pgxscan.NewScanner(entityRows).Scan(&model); err != nil {
		return nil, database.GetError(fmt.Errorf("could not scan device entities into struct: %w", err))
	}

	return model, nil
}
