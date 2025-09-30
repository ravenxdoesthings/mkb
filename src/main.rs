use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::esi::{Claims, Token, processor::Job};

mod config;
mod esi;

#[derive(Clone, Debug)]
struct User {
    character_id: i64,
    access_token: String,
    refresh_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl User {
    fn new(access_token: String, refresh_token: String, claims: Claims) -> Self {
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

#[derive(Clone)]
struct UserStore {
    users: Arc<RwLock<HashMap<i64, User>>>,
}

impl UserStore {
    fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[derive(Clone)]
struct AppState {
    user_store: UserStore,
    client: esi::EsiClient,
}

impl AppState {
    fn new(store: UserStore, client: &esi::EsiClient) -> Self {
        Self {
            user_store: store,
            client: client.clone(),
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up tracing subscriber to log to stdout
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            "debug,hyper=off,reqwest=off",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::Config::from_env();
    let client = esi::EsiClient::from_config(config.clone());

    let (jobs_sender, jobs_receiver) = tokio::sync::mpsc::channel(100);

    let processor = esi::processor::Processor::new(&config.database_uri, &client);
    let _ = processor.start(jobs_receiver).await;

    let user_store = UserStore::new();

    let state = AppState::new(user_store, &client);

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
            tracing::info!("hello");
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
        .route("/", get(index))
        .route("/auth", get(auth))
        .route("/auth/callback", get(callback))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    jobs_sender.send(Job::Stop).await.unwrap();
}

async fn index() -> impl IntoResponse {
    (StatusCode::OK, "Hello, World!")
}

async fn auth(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let (url, nonce) = state.client.build_auth_url();

    (
        StatusCode::OK,
        jar.add(Cookie::build(("mkb_state", nonce.clone())).path("/")),
        Html(format!(
            r#"<a href="{url}">Authenticate with EVE Online</a>"#
        )),
    )
}

async fn callback(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    jar: CookieJar,
) -> impl IntoResponse {
    tracing::debug!(params = format!("{params:?}"));

    let nonce = match jar.get("mkb_state") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [("Content-Type", "text/plain")],
                "Missing state cookie".to_string(),
            );
        }
    };

    if params.get("state") != Some(&nonce) {
        return (
            StatusCode::BAD_REQUEST,
            [("Content-Type", "text/html")],
            "Invalid state".to_string(),
        );
    }

    let code = params.get("code").unwrap_or(&String::new()).to_owned();

    let _ = state
        .client
        .clone()
        .token_exchange(Token::AuthCode(code))
        .await
        .map_err(|e| {
            tracing::error!(error = e.to_string(), "Failed to exchange token");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/html")],
                e.to_string(),
            )
        })
        .map(|user| {
            state
                .user_store
                .users
                .write()
                .unwrap()
                .insert(user.character_id, user);
        });

    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        "".to_string(),
    )
}
