package repository

import (
	"encoding/json"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/JacobSoderblom/krypin/pkg/timestamp"
	"github.com/gofrs/uuid"
	"github.com/jackc/pgtype"
)

type deviceModel struct {
	ID              uuid.UUID          `db:"id"`
	Name            pgtype.Varchar     `db:"name"`
	Manufacturer    pgtype.Varchar     `db:"manufacturer"`
	Model           pgtype.Varchar     `db:"model"`
	SoftwareVersion pgtype.Varchar     `db:"sw_ver"`
	Identifier      pgtype.Varchar     `db:"identifier"`
	CreatedAt       pgtype.Timestamptz `db:"created_at"`
	UpdatedAt       pgtype.Timestamptz `db:"updated_at"`
	Connections     pgtype.JSONB       `db:"connections"`
	Entities        []*deviceEntity    `db:"-"`
}

type deviceEntity struct {
	ID        pgtype.Text           `db:"id"`
	Type      pgtype.Varchar        `db:"type"`
	Name      pgtype.Varchar        `db:"name"`
	Module    pgtype.Text           `db:"module"`
	Features  database.VarcharArray `db:"features"`
	CreatedAt pgtype.Timestamptz    `db:"created_at" json:"created_at"`
	UpdatedAt pgtype.Timestamptz    `db:"updated_at" json:"updated_at"`
	DeviceID  uuid.UUID             `db:"device_id" json:"device_id"`
	States    []*entityState        `db:"-"`
}

type entityState struct {
	Value     pgtype.JSONB       `db:"value"`
	EntityID  pgtype.Text        `db:"entity_id" json:"entity_id"`
	CreatedAt pgtype.Timestamptz `db:"created_at" json:"created_at"`
}

func toDeviceModel(d *devicereg.Device) *deviceModel {
	m := &deviceModel{
		ID:              d.ID,
		Identifier:      pgtype.Varchar{String: d.Identifier, Status: pgtype.Present},
		Manufacturer:    pgtype.Varchar{String: d.Manufacturer, Status: pgtype.Present},
		Model:           pgtype.Varchar{String: d.Model, Status: pgtype.Present},
		SoftwareVersion: pgtype.Varchar{String: d.SoftwareVersion, Status: pgtype.Present},
		Name:            pgtype.Varchar{String: d.Name, Status: pgtype.Present},
	}

	if len(d.Connections) > 0 {
		b, _ := json.Marshal(&d.Connections)
		m.Connections = pgtype.JSONB{Bytes: b, Status: pgtype.Present}
	}

	for _, e := range d.Entities {
		m.Entities = append(m.Entities, toEntityModel(e))
	}

	return m
}

func toEntityModel(e *devicereg.Entity) *deviceEntity {
	m := &deviceEntity{
		ID:       pgtype.Text{String: e.ID, Status: pgtype.Present},
		DeviceID: e.DeviceID,
		Name:     pgtype.Varchar{String: e.Name, Status: pgtype.Present},
		Type:     pgtype.Varchar{String: e.Type, Status: pgtype.Present},
		Module:   pgtype.Text{String: e.Module, Status: pgtype.Present},
		Features: database.NewVarcharArray(len(e.Features)),
	}

	if len(e.Features) > 0 {
		m.Features.Status = pgtype.Present

		for _, f := range e.Features {
			m.Features.Elements = append(m.Features.Elements, pgtype.Varchar{String: f, Status: pgtype.Present})
		}
	}

	for _, s := range e.States {
		m.States = append(m.States, toEntityStateModel(s))
	}

	return m
}

func toEntityStateModel(s *devicereg.EntityState) *entityState {
	b, _ := json.Marshal(s.Value)
	return &entityState{
		EntityID: pgtype.Text{String: s.EntityID, Status: pgtype.Present},
		Value:    pgtype.JSONB{Bytes: b, Status: pgtype.Present},
	}
}

func fromDeviceModel(m *deviceModel) *devicereg.Device {
	d := &devicereg.Device{
		ID:              m.ID,
		Name:            m.Name.String,
		Manufacturer:    m.Manufacturer.String,
		Model:           m.Model.String,
		Identifier:      m.Identifier.String,
		SoftwareVersion: m.SoftwareVersion.String,
		CreatedAt:       timestamp.Timestamp{Time: m.CreatedAt.Time},
		UpdatedAt:       timestamp.Timestamp{Time: m.UpdatedAt.Time},
	}

	if m.Connections.Status == pgtype.Present {
		json.Unmarshal(m.Connections.Bytes, &m.Connections)
	}

	for _, e := range m.Entities {
		d.Entities = append(d.Entities, fromDeviceEntity(e))
	}

	return d
}

func fromDeviceModelSlice(models []*deviceModel) []*devicereg.Device {
	res := []*devicereg.Device{}

	for _, m := range models {
		res = append(res, fromDeviceModel(m))
	}

	return res
}

func fromDeviceEntity(m *deviceEntity) *devicereg.Entity {
	e := &devicereg.Entity{
		ID:        m.ID.String,
		Name:      m.Name.String,
		DeviceID:  m.DeviceID,
		Type:      m.Type.String,
		Module:    m.Module.String,
		States:    []*devicereg.EntityState{},
		CreatedAt: timestamp.Timestamp{Time: m.CreatedAt.Time},
		UpdatedAt: timestamp.Timestamp{Time: m.UpdatedAt.Time},
	}

	for _, f := range m.Features.Elements {
		e.Features = append(e.Features, f.String)
	}

	for _, s := range m.States {
		e.States = append(e.States, fromEntityStateModel(s))
	}

	return e
}

func fromEntityStateModel(m *entityState) *devicereg.EntityState {
	s := &devicereg.EntityState{
		CreatedAt: &timestamp.Timestamp{Time: m.CreatedAt.Time},
		EntityID:  m.EntityID.String,
	}

	var val map[string]interface{}
	json.Unmarshal(m.Value.Bytes, &val)

	s.Value = val

	return s
}

func addStatesToEntity(entities []*deviceEntity, states ...*entityState) {
	for i := range entities {
		for _, s := range states {
			if s.EntityID.String != entities[i].ID.String {
				continue
			}

			entities[i].States = append(entities[i].States, s)
		}
	}
}
