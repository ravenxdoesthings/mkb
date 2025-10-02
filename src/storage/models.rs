use diesel::prelude::*;

use crate::esi::Claims;

#[derive(Clone, Debug, Queryable, Insertable, Selectable)]
#[diesel(table_name = super::schema::killmails)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Killmail {
    pub killmail_id: i64,
    pub killmail_hash: String,
    pub status: String,
}

#[derive(Clone, Debug, Queryable, Insertable, Selectable)]
#[diesel(table_name = super::schema::entities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub type_: String,
}

#[derive(Clone, Debug, Queryable, Insertable, Selectable)]
#[diesel(table_name = super::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: uuid::Uuid,
    pub character_id: i64,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_fetched: Option<chrono::DateTime<chrono::Utc>>,
}

impl User {
    pub fn new(access_token: String, refresh_token: String, claims: Claims) -> Self {
        let character_id: i64 = claims
            .sub
            .replace("CHARACTER:EVE:", "")
            .parse()
            .unwrap_or(0);

        Self {
            id: uuid::Uuid::new_v4(),
            character_id,
            access_token,
            refresh_token,
            expires_at: chrono::DateTime::from_timestamp(claims.exp, 0).unwrap_or_default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_fetched: None,
        }
    }
}
