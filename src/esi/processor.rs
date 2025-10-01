use diesel::dsl::{IntervalDsl, now};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sql_types::Timestamptz;
use diesel::{ExpressionMethods, IntoSql, QueryDsl, RunQueryDsl};

use crate::esi::EsiClient;
use crate::storage::handlers::{save_killmail, save_user};
use crate::storage::schema::users::expires_at;
use crate::storage::{models, schema};

pub enum Job {
    Refresh,
    Killmails,
    Killmail(i64, String),
    Character(i64),
    Corporation(i64),
    Alliance(i64),
    SaveCharacter(models::User),
    SaveKillmail(models::Killmail),
    Stop,
}

pub struct Processor {
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub client: EsiClient,
}

impl Processor {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>, client: &EsiClient) -> Self {
        Processor {
            pool,
            client: client.clone(),
        }
    }

    pub async fn start(&self, mut jobs_rx: tokio::sync::mpsc::Receiver<Job>) {
        let pool = self.pool.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            while let Some(job) = jobs_rx.recv().await {
                match job {
                    Job::Refresh => {
                        let mut conn = pool.get().unwrap();
                        let users = match schema::users::dsl::users
                            .filter(expires_at.gt(now.into_sql::<Timestamptz>() - 20.minutes()))
                            .load::<models::User>(&mut conn)
                        {
                            Ok(users) => users,
                            Err(e) => {
                                tracing::error!(error = e.to_string(), "Failed to refresh users");
                                continue;
                            }
                        };

                        client.refresh(users).await;
                    }
                    Job::Killmails => {
                        let mut conn = pool.get().unwrap();
                        let users = match schema::users::dsl::users.load::<models::User>(&mut conn)
                        {
                            Ok(users) => users,
                            Err(e) => {
                                tracing::error!(error = e.to_string(), "Failed to refresh users");
                                continue;
                            }
                        };

                        client.get_killmails(users).await;
                    }
                    Job::Killmail(killmail_id, killmail_hash) => {
                        tracing::info!(killmail_id, killmail_hash, "processing killmail");
                        // Here you would typically fetch the killmail data from ESI and store it in the database
                    }
                    Job::Character(character_id) => {
                        tracing::info!(character_id, "resolving character ID");
                        // Fetch and store character data
                    }
                    Job::Corporation(corporation_id) => {
                        tracing::info!(corporation_id, "resolving corporation ID");
                        // Fetch and store corporation data
                    }
                    Job::Alliance(alliance_id) => {
                        tracing::info!(alliance_id, "resolving alliance ID");
                        // Fetch and store alliance data
                    }
                    Job::SaveCharacter(user) => {
                        tracing::debug!(character_id = user.character_id, "saving user");
                        // Save or update the user in the database
                        if let Err(e) = save_user(&pool, user) {
                            tracing::error!(error = e.to_string(), "Failed to save user");
                        };
                    }
                    Job::SaveKillmail(killmail) => {
                        tracing::debug!(
                            killmail_id = killmail.killmail_id,
                            killmail_hash = killmail.killmail_hash,
                            "saving killmail"
                        );
                        if let Err(e) = save_killmail(&pool, killmail) {
                            tracing::error!(error = e.to_string(), "Failed to save killmail");
                        }
                    }
                    Job::Stop => {
                        tracing::info!("Stopping processor.");
                        break;
                    }
                }
            }
        });
    }
}
