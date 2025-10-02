use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use reqwest::StatusCode;

use crate::esi;
use crate::http::state::AppState;

pub async fn index() -> impl IntoResponse {
    (StatusCode::OK, "Hello, World!")
}

pub async fn auth(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let (url, nonce) = state.client.build_auth_url();

    (
        StatusCode::OK,
        jar.add(Cookie::build(("mkb_state", nonce.clone())).path("/")),
        Html(format!(
            r#"<a href="{url}">Authenticate with EVE Online</a>"#
        )),
    )
}

pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    jar: CookieJar,
) -> impl IntoResponse {
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
        .token_exchange(esi::Token::AuthCode(code))
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
            if state
                .jobs_sender
                .try_send(esi::processor::Job::SaveCharacter(user.clone()))
                .is_err()
            {
                tracing::error!(
                    user = format!("{user:?}"),
                    "Failed to enqueue user save job"
                );
            }
        });

    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        "".to_string(),
    )
}

pub async fn refresh(State(state): State<AppState>) -> impl IntoResponse {
    state
        .jobs_sender
        .try_send(esi::processor::Job::Refresh)
        .map_err(|e| {
            tracing::error!(error = e.to_string(), "Failed to enqueue refresh job");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/html")],
                e.to_string(),
            )
        })
        .ok();

    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        "".to_string(),
    )
}

pub async fn killmails(State(state): State<AppState>) -> impl IntoResponse {
    state
        .jobs_sender
        .try_send(esi::processor::Job::Killmails)
        .map_err(|e| {
            tracing::error!(error = e.to_string(), "Failed to enqueue refresh job");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/html")],
                e.to_string(),
            )
        })
        .ok();

    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        "".to_string(),
    )
}

pub async fn resolve(State(state): State<AppState>) -> impl IntoResponse {
    state
        .jobs_sender
        .try_send(esi::processor::Job::ResolveKillmails)
        .map_err(|e| {
            tracing::error!(error = e.to_string(), "Failed to enqueue refresh job");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "text/html")],
                e.to_string(),
            )
        })
        .ok();

    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        "".to_string(),
    )
}
