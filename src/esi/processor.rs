use std::collections::HashMap;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::esi::EsiClient;
use crate::models::User;

pub enum Job {
    Refresh,
    Killmails,
    Killmail(i64, String),
    Character(i64),
    Corporation(i64),
    Alliance(i64),
    Save(User),
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
        let _pool = self.pool.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            while let Some(job) = jobs_rx.recv().await {
                match job {
                    Job::Refresh => {
                        // get users to refresh then call refresh method
                        let users = HashMap::new();
                        let _ = client.refresh(users).await;
                        // Here you would typically refresh tokens for all users in the database
                    }
                    Job::Killmails => {
                        tracing::info!("Processing killmails job");
                        // Here you would typically fetch new killmails from ESI and enqueue them for processing
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
                    Job::Save(user) => {
                        tracing::info!(character_id = user.character_id, "saving user");
                        // Save or update the user in the database
                        let _ = user;
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
