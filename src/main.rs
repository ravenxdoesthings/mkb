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
    let client = esi::EsiClient::from_config(config.clone());

    let (jobs_sender, jobs_receiver) = tokio::sync::mpsc::channel(100);

    let mut conn = pool.get()?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::format_err!("failed to apply migrations: {e}"))?;

    let processor = esi::processor::Processor::new(pool, &client);
    let _ = processor.start(jobs_receiver).await;

    let state = AppState::new(jobs_sender.clone(), &client);

    let refresh_send = jobs_sender.clone();
    tokio::spawn(async move {
        loop {
            let tx = refresh_send.clone();
            let _ = tx.send(Job::Refresh).await;

            let _ = tokio::time::sleep(std::time::Duration::from_secs(60 * 5)).await;
        }
    });

    let fetch_send = jobs_sender.clone();
    tokio::spawn(async move {
        loop {
            let tx = fetch_send.clone();
            let _ = tx.send(Job::Killmails).await;

            let _ = tokio::time::sleep(std::time::Duration::from_secs(60 * 10)).await;
        }
    });

    let resolve_send = jobs_sender.clone();
    tokio::spawn(async move {
        loop {
            let tx = resolve_send.clone();

            // just to not mark them unused for now
            let _ = tx.send(Job::Killmail(1, "test".into())).await;
            let _ = tx.send(Job::Character(1)).await;
            let _ = tx.send(Job::Corporation(1)).await;
            let _ = tx.send(Job::Alliance(1)).await;

            let _ = tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
        }
    });

    // Build a simple Axum app
    let app = Router::new()
        .route("/", get(handlers::index))
        .route("/auth", get(handlers::auth))
        .route("/auth/callback", get(handlers::callback))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    jobs_sender.send(Job::Stop).await?;

    Ok(())
}
