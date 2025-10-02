-- Your SQL goes here
CREATE TABLE users (
	id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
	character_id BIGINT UNIQUE NOT NULL,
	access_token TEXT NOT NULL,
	refresh_token TEXT NOT NULL,
	expires_at TIMESTAMPTZ NOT NULL,
	created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
	updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
	last_fetched TIMESTAMPTZ
);
