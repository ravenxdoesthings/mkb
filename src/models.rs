use diesel::prelude::*;

use crate::esi::Claims;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::killmails)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Killmail {
    pub id: uuid::Uuid,
    pub killmail_id: i64,
    pub killmail_hash: String,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct User {
    pub character_id: i64,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl User {
    pub fn new(access_token: String, refresh_token: String, claims: Claims) -> Self {
        let character_id: i64 = claims
            .sub
            .replace("CHARACTER:EVE:", "")
            .parse()
            .unwrap_or(0);

        Self {
            character_id,
            access_token,
            refresh_token,
            expires_at: chrono::DateTime::from_timestamp(claims.exp, 0).unwrap_or_default(),
        }
    }
}
