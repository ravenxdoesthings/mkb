use std::time::Duration;

use axum::{Router, routing::get};
use diesel::{PgConnection, r2d2::ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use r2d2::Pool;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use mkb::{
    esi::{self, processor::Job},
    http::{handlers, state::AppState},
};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    // Set up tracing subscriber to log to stdout
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            "debug,hyper=off,reqwest=off",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = mkb::config::Config::from_env();

    let manager = ConnectionManager::<PgConnection>::new(config.database_uri.clone());
    let pool = Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");

    let (jobs_sender, jobs_receiver) = tokio::sync::mpsc::channel(10000);

    let client = esi::EsiClient::from_config(&jobs_sender, config.clone());

    let mut conn = pool.get()?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::format_err!("failed to apply migrations: {e}"))?;

    let processor = esi::processor::Processor::new(pool, &client);
    let _ = processor.start(jobs_receiver).await;

    let state = AppState::new(jobs_sender.clone(), &client);

    let (scheduler_stop_tx, scheduler_stop_rx) = tokio::sync::oneshot::channel();
    let scheduler_handle =
        mkb::esi::scheduler::start_scheduler(scheduler_stop_rx, jobs_sender.clone()).await;

    // Build a simple Axum app
    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/auth", get(handlers::auth))
        .route("/auth/callback", get(handlers::callback))
        .route("/testing/refresh", get(handlers::refresh))
        .route("/testing/killmails", get(handlers::killmails))
        .route("/testing/resolve", get(handlers::resolve))
        .with_state(state);

    tracing::info!("starting server... http://localhost:3000/auth");

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    jobs_sender.send(Job::Stop).await?;

    let _ = scheduler_stop_tx.send(());
    tokio::time::interval(Duration::from_secs(3)).tick().await;
    if !scheduler_handle.is_finished() {
        scheduler_handle.abort();
        let _ = scheduler_handle.await;
    }

    Ok(())
}
