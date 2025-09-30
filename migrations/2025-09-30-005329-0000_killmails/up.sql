-- Your SQL goes here
CREATE TABLE killmails (
	id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
	killmail_id BIGINT UNIQUE NOT NULL,
	killmail_hash TEXT NOT NULL,
	status TEXT NOT NULL DEFAULT 'pending'
);
