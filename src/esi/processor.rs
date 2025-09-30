use std::collections::HashMap;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};

use crate::esi::EsiClient;

pub enum Job {
    Refresh,
    Killmails,
    Killmail(i64, String),
    Character(i64),
    Corporation(i64),
    Alliance(i64),
    Stop,
}

pub struct Processor {
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub client: EsiClient,
}

impl Processor {
    pub fn new(database_url: &str, client: &EsiClient) -> Self {
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        let pool = Pool::builder()
            .build(manager)
            .expect("Failed to create pool.");
        Processor {
            pool,
            client: client.clone(),
        }
    }

    pub fn start(&self, mut jobs_rx: tokio::sync::mpsc::Receiver<Job>) {
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
                        println!("Processing killmails job");
                        // Here you would typically fetch new killmails from ESI and enqueue them for processing
                    }
                    Job::Killmail(killmail_id, killmail_hash) => {
                        println!("Processing killmail: {} {}", killmail_id, killmail_hash);
                        // Here you would typically fetch the killmail data from ESI and store it in the database
                    }
                    Job::Character(character_id) => {
                        println!("Processing character: {}", character_id);
                        // Fetch and store character data
                    }
                    Job::Corporation(corporation_id) => {
                        println!("Processing corporation: {}", corporation_id);
                        // Fetch and store corporation data
                    }
                    Job::Alliance(alliance_id) => {
                        println!("Processing alliance: {}", alliance_id);
                        // Fetch and store alliance data
                    }
                    Job::Stop => {
                        println!("Stopping processor.");
                        break;
                    }
                }
            }
        });
    }
}
