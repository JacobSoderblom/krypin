CREATE TABLE IF NOT EXISTS areas (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    parent UUID REFERENCES areas(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS devices (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    adapter TEXT NOT NULL,
    manufacturer TEXT,
    model TEXT,
    sw_version TEXT,
    hw_version TEXT,
    area UUID REFERENCES areas(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS entities (
    id UUID PRIMARY KEY,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    domain TEXT NOT NULL,
    icon TEXT,
    key TEXT,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS entity_states (
    id BIGSERIAL PRIMARY KEY,
    entity_id UUID NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    value JSONB NOT NULL,
    attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_changed TIMESTAMPTZ NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL,
    source TEXT
);

CREATE INDEX IF NOT EXISTS idx_entity_states_entity_id ON entity_states(entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_states_entity_time ON entity_states(entity_id, last_updated DESC);
