-- Your SQL goes here
CREATE TABLE killmails_x_entities (
	id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
	killmail_id UUID NOT NULL REFERENCES killmails(id),
	entity_id BIGINT NOT NULL REFERENCES entities(id),
	entity_side TEXT NOT NULL
);
