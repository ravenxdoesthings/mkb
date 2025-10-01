// @generated automatically by Diesel CLI.

diesel::table! {
    entities (id) {
        id -> Int8,
        name -> Text,
        #[sql_name = "type"]
        type_ -> Text,
    }
}

diesel::table! {
    killmails (killmail_id) {
        killmail_id -> Int8,
        killmail_hash -> Text,
        status -> Text,
    }
}

diesel::table! {
    killmails_x_entities (id) {
        id -> Uuid,
        killmail_id -> Int8,
        entity_id -> Int8,
        entity_side -> Text,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        character_id -> Int8,
        access_token -> Text,
        refresh_token -> Text,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        last_fetched -> Nullable<Timestamptz>,
    }
}

diesel::joinable!(killmails_x_entities -> entities (entity_id));
diesel::joinable!(killmails_x_entities -> killmails (killmail_id));

diesel::allow_tables_to_appear_in_same_query!(entities, killmails, killmails_x_entities, users,);
