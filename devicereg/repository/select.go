package repository

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/JacobSoderblom/krypin/internal/errors"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/Masterminds/squirrel"
	"github.com/gofrs/uuid"
	"github.com/jackc/pgtype"
	"github.com/jackc/pgx/v4"
	"github.com/randallmlough/pgxscan"
)

func (r *deviceRepository) Select(ctx context.Context, deviceID uuid.UUID) (*devicereg.Device, error) {
	devices, err := r.selectDevices(ctx, &deviceID, nil)
	if err != nil {
		return nil, err
	}

	if len(devices) == 0 {
		return nil, errors.New(errors.NotFound, fmt.Sprintf("device with id (%s) was not found", deviceID))
	}

	return devices[0], nil
}

func (r *deviceRepository) SelectAll(ctx context.Context) ([]*devicereg.Device, error) {
	return r.selectDevices(ctx, nil, nil)
}

func (r *deviceRepository) SelectByIdentifier(ctx context.Context, identifier string) (*devicereg.Device, error) {
	devices, err := r.selectDevices(ctx, nil, &identifier)
	if err != nil {
		return nil, err
	}

	if len(devices) == 0 {
		return nil, errors.New(errors.NotFound, fmt.Sprintf("device with identifier (%s) was not found", identifier))
	}

	return devices[0], nil
}

func (r *deviceRepository) SelectLatestStateForEntities(ctx context.Context, IDs ...string) ([]*devicereg.EntityState, error) {
	states, err := r.selectLatestStateForEntities(ctx, IDs...)
	if err != nil {
		return nil, err
	}

	res := []*devicereg.EntityState{}

	for _, s := range states {
		res = append(res, fromEntityStateModel(s))
	}

	return res, nil
}

func (r *deviceRepository) selectDevices(ctx context.Context, deviceID *uuid.UUID, identifier *string) ([]*devicereg.Device, error) {
	q, err := selectDevice(deviceID, identifier)
	if err != nil {
		return nil, database.GetError(fmt.Errorf("could not create sql for selecting all devices: %w", err))
	}

	sql, args, err := q.ToSql()
	if err != nil {
		return nil, database.GetError(fmt.Errorf("could not create sql for selecting all devices: %w", err))
	}

	tx := r.db.Tx(ctx)

	rows, err := tx.Query(ctx, sql, args...)
	if err != nil {
		return nil, database.GetError(fmt.Errorf("failed to select devices: %w", err))
	}

	type extendedDevice struct {
		deviceModel
		Entities pgtype.JSON `db:"entities"`
		States   pgtype.JSON `db:"states"`
	}

	var devices []*extendedDevice

	if err = pgxscan.NewScanner(rows).Scan(&devices); err != nil {
		return nil, database.GetError(fmt.Errorf("failed to scan devices: %w", err))
	}

	res := []*deviceModel{}

	for _, d := range devices {
		m := &d.deviceModel

		entities := []*deviceEntity{}
		if err = json.Unmarshal(d.Entities.Bytes, &entities); err != nil {
			return nil, err
		}

		states := []*entityState{}
		if err = json.Unmarshal(d.States.Bytes, &states); err != nil {
			return nil, err
		}

		if len(states) > 0 {
			addStatesToEntity(entities, states...)
		}

		m.Entities = entities

		res = append(res, m)
	}

	return fromDeviceModelSlice(res), nil
}

func (r *deviceRepository) scanEntitiesFromBatch(res pgx.BatchResults) ([]*deviceEntity, error) {
	entityRows, err := res.Query()
	if err != nil {
		return nil, database.GetError(fmt.Errorf("failed to select device entities: %w", err))
	}

	var entities []*deviceEntity

	if err = pgxscan.NewScanner(entityRows).Scan(&entities); err != nil {
		if database.IsNoRows(err) {
			return []*deviceEntity{}, nil
		}
		return nil, database.GetError(fmt.Errorf("could not scan device entities into struct: %w", err))
	}

	return entities, nil
}

func (r *deviceRepository) selectLatestStateForEntities(ctx context.Context, IDs ...string) ([]*entityState, error) {
	states := []*entityState{}

	batch := &pgx.Batch{}

	for _, id := range IDs {
		entityID := pgtype.Text{String: id, Status: pgtype.Present}
		stateSQL, stateArgs, err := selectEntityStateQuery(&entityID, nil).Limit(1).ToSql()
		if err != nil {
			return nil, database.GetError(fmt.Errorf("could not create sql for selecting latest entity state: %w", err))
		}

		batch.Queue(stateSQL, stateArgs...)
	}

	if batch.Len() == 0 {
		return states, nil
	}

	err := r.db.BatchTx(ctx, batch, func(res pgx.BatchResults) error {
		for i := 0; i < batch.Len(); i++ {
			var s entityState

			rows, err := res.Query()
			if err != nil {
				return database.GetError(fmt.Errorf("failed to select entity state: %w", err))
			}

			if err := pgxscan.NewScanner(rows).Scan(&s); err != nil {
				if database.IsNoRows(err) {
					continue
				}
				return database.GetError(fmt.Errorf("could not scan entity state into struct: %w", err))
			}

			states = append(states, &s)
		}

		return nil
	})
	if err != nil {
		return nil, err
	}

	return states, nil
}

func selectDevice(deviceID *uuid.UUID, identifier *string) (squirrel.SelectBuilder, error) {
	distinctStates := sq.
		Select("DISTINCT ON (entity_id) entity_id", "created_at", "value").
		From("entity_states").
		OrderBy("entity_id, created_at DESC")

	sql, args, err := sq.
		Select("entity_id", "created_at", "value").
		FromSelect(distinctStates, "distinctStates").
		OrderBy("created_at").
		ToSql()
	if err != nil {
		return sq.Select(), err
	}

	entityAgg := "json_agg(json_build_object('id', de.id, 'name', de.name, 'features', de.features, 'device_id', de.device_id, 'module', de.module, 'created_at', de.created_at, 'updated_at', de.updated_at))"
	stateAgg := "json_agg(json_build_object('value', es.value, 'entity_id', es.entity_id, 'created_at', es.created_at))"

	q := sq.
		Select("d.*", fmt.Sprintf("%s as entities", entityAgg), fmt.Sprintf("%s as states", stateAgg)).
		From("devices d").
		LeftJoin("device_entities de ON de.device_id = d.id").
		LeftJoin(fmt.Sprintf("(%s) as es ON es.entity_id = de.id", sql), args...).
		GroupBy("d.id")

	if deviceID != nil {
		return q.Where(squirrel.Eq{
			"d.id": deviceID,
		}).Limit(1), nil
	}

	if identifier != nil {
		return q.Where(squirrel.Eq{
			"d.identifier": identifier,
		}).Limit(1), nil
	}

	return q, nil
}

func selectDeviceQuery(deviceID *uuid.UUID, identifier *string) squirrel.SelectBuilder {
	q := sq.Select("id", "name", "manufacturer", "model", "sw_ver", "identifier", "created_at", "updated_at").
		From("devices d").
		Limit(1)

	if deviceID != nil {
		return q.Where(squirrel.Eq{
			"id": deviceID,
		})
	}

	if identifier != nil {
		return q.Where(squirrel.Eq{
			"identifier": identifier,
		})
	}

	return q
}

func selectEntitiesQuery(deviceID *uuid.UUID, identifier *string) squirrel.SelectBuilder {
	q := sq.Select("de.id", "de.name", "de.type", "de.module", "de.features", "de.created_at", "de.updated_at", "de.device_id").
		From("device_entities de")

	if deviceID != nil {
		return q.Where(squirrel.Eq{
			"de.device_id": deviceID,
		})
	}

	if identifier != nil {
		return q.Join("devices d ON de.device_id = d.id").Where(squirrel.Eq{
			"d.identifier": identifier,
		})
	}

	return q
}

func selectEntityStateQuery(entityID *pgtype.Text, deviceID *uuid.UUID) squirrel.SelectBuilder {

	q := sq.Select("es.value", "es.created_at", "es.entity_id").
		From("entity_states es").
		OrderBy("created_at DESC")

	if entityID != nil {
		return q.
			Where(squirrel.Eq{
				"es.entity_id": entityID,
			}).
			OrderBy("created_at DESC")
	}

	if deviceID != nil {
		return q.
			Join("device_entities de ON de.id = es.entity_id").
			Where(squirrel.Eq{
				"de.device_id": deviceID,
			})
	}

	return q
}
