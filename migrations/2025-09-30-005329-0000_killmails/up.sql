-- Your SQL goes here
CREATE TABLE killmails (
	killmail_id BIGINT PRIMARY KEY NOT NULL,
	killmail_hash TEXT NOT NULL,
	status TEXT NOT NULL DEFAULT 'pending'
);
