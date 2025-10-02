use super::{models, schema};
use diesel::insert_into;
use diesel::prelude::*;
use diesel::{PgConnection, r2d2::ConnectionManager};
use r2d2::Pool;

pub fn save_user(
    pool: &Pool<ConnectionManager<PgConnection>>,
    user: models::User,
) -> Result<usize, diesel::result::Error> {
    let mut conn = pool.get().unwrap();
    insert_into(schema::users::table)
        .values(&user)
        .on_conflict(schema::users::character_id)
        .do_update()
        .set((
            schema::users::access_token.eq(&user.access_token),
            schema::users::refresh_token.eq(&user.refresh_token),
            schema::users::expires_at.eq(&user.expires_at),
            schema::users::updated_at.eq(chrono::Utc::now()),
        ))
        .execute(&mut conn)
}

pub fn save_killmail(
    pool: &Pool<ConnectionManager<PgConnection>>,
    killmail: models::Killmail,
) -> Result<usize, diesel::result::Error> {
    let mut conn = pool.get().unwrap();
    insert_into(schema::killmails::table)
        .values(&killmail)
        .on_conflict_do_nothing()
        .execute(&mut conn)
}

pub fn set_killmail_status(
    pool: &Pool<ConnectionManager<PgConnection>>,
    killmail_id: i64,
    status: &str,
) -> Result<usize, diesel::result::Error> {
    let mut conn = pool.get().unwrap();
    diesel::update(schema::killmails::table.filter(schema::killmails::killmail_id.eq(killmail_id)))
        .set(schema::killmails::status.eq(status))
        .execute(&mut conn)
}

pub fn save_entity(
    pool: &Pool<ConnectionManager<PgConnection>>,
    entity: models::Entity,
) -> Result<usize, diesel::result::Error> {
    let mut conn = pool.get().unwrap();
    insert_into(schema::entities::table)
        .values(&entity)
        .on_conflict_do_nothing()
        .execute(&mut conn)
}
