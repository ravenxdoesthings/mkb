use std::collections::HashMap;

use crate::storage::models;
use crate::{config::Config, storage::models::Killmail};
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode, jwk::Jwk};
use reqwest::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub mod processor;
pub mod scheduler;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Claims {
    pub aud: Vec<String>,
    pub exp: i64,
    pub iss: String,
    pub sub: String,
}

#[derive(Clone)]
pub struct EsiClient {
    app_id: String,
    app_secret: String,
    redirect_uri: String,
    jobs_sender: tokio::sync::mpsc::Sender<processor::Job>,
}

#[derive(Clone, Debug, Deserialize)]
struct KillmailItem {
    killmail_hash: String,
    killmail_id: i64,
}

impl EsiClient {
    pub fn from_config(sender: &tokio::sync::mpsc::Sender<processor::Job>, config: Config) -> Self {
        Self {
            app_id: config.application_id,
            app_secret: config.application_secret,
            redirect_uri: config.redirect_uri,
            jobs_sender: sender.clone(),
        }
    }
}

pub enum Token {
    AuthCode(String),
    RefreshToken(String),
}

impl EsiClient {
    pub fn build_auth_url(&self) -> (String, String) {
        let nonce = Uuid::new_v4().to_string();
        let params = HashMap::from([
            ("response_type", "code"),
            ("client_id", self.app_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            (
                "scope",
                "publicData esi-killmails.read_killmails.v1 esi-killmails.read_corporation_killmails.v1",
            ),
            ("state", &nonce),
        ]);

        let mut url = "https://login.eveonline.com/v2/oauth/authorize?".to_string();
        let params_str = params
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<String>>()
            .join("&");
        url.push_str(&params_str);

        (url, nonce)
    }

    pub async fn token_exchange(&self, token: Token) -> Result<models::User, anyhow::Error> {
        let basic_auth = self.build_basic_auth();
        let payload = self.build_payload(token);

        let response = match reqwest::Client::new()
            .post("https://login.eveonline.com/v2/oauth/token")
            .header("Authorization", format!("Basic {basic_auth}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&payload)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Err(anyhow::format_err!("failed to send request: {e}"));
            }
        };

        let result = match Self::json(response).await {
            Ok(json) => json,
            Err(e) => {
                return Err(anyhow::format_err!("failed to decode JSON: {e}"));
            }
        };

        let access_token = result["access_token"]
            .to_string()
            .trim_matches('"')
            .to_string();
        let refresh_token = result["refresh_token"]
            .to_string()
            .trim_matches('"')
            .to_string();

        match self.validate_jwt(&access_token).await {
            Ok(claims) => {
                let user = models::User::new(access_token, refresh_token, claims);
                tracing::trace!(
                    user.access_token,
                    user.refresh_token,
                    expires_at = format!("{}", user.expires_at),
                    "validated token"
                );
                Ok(user)
            }
            Err(e) => Err(anyhow::format_err!("failed to validate JWT: {e}")),
        }
    }

    fn build_basic_auth(&self) -> String {
        base64::engine::general_purpose::URL_SAFE
            .encode(format!("{}:{}", self.app_id, self.app_secret))
    }

    fn build_payload(&self, token: Token) -> HashMap<String, String> {
        match token {
            Token::AuthCode(code) => HashMap::from([
                ("grant_type".into(), "authorization_code".into()),
                ("code".into(), code.clone()),
            ]),
            Token::RefreshToken(refresh_token) => HashMap::from([
                ("grant_type".into(), "refresh_token".into()),
                ("refresh_token".into(), refresh_token.clone()),
            ]),
        }
    }

    pub async fn validate_jwt(&self, token: &String) -> Result<Claims, anyhow::Error> {
        tracing::trace!(token, "attempting to validate token");
        let decoding_key = Self::get_rsa256_key().await.unwrap();
        let mut validations = Validation::new(Algorithm::RS256);
        validations.required_spec_claims = vec![String::from("sub")].into_iter().collect();
        let aud = vec![self.app_id.clone(), "EVE Online".to_string()];
        validations.set_audience(&aud);

        let token = token.trim_matches('"').to_string();

        let token: TokenData<Value> = decode(token.as_str(), &decoding_key, &validations)?;
        if token.claims["iss"].as_str().unwrap() != "login.eveonline.com"
            && token.claims["iss"].as_str().unwrap() != "https://login.eveonline.com"
        {
            return Err(anyhow::format_err!("JWT issuer is incorrect",));
        }

        let token_claims = serde_json::from_value(token.claims)?;
        Ok(token_claims)
    }

    pub async fn get_rsa256_key() -> Result<DecodingKey, anyhow::Error> {
        let jwks_uri = "https://login.eveonline.com/oauth/jwks";
        let resp: Value = reqwest::get(jwks_uri).await?.json().await?;

        let key = resp["keys"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|key| key["alg"] == "RS256")
            .map(|key| {
                serde_json::to_string(key)
                    .map_err(|e| anyhow::format_err!("Failed to serialize key: {}", e))
            })
            .next()
            .ok_or_else(|| anyhow::format_err!("No RS256 key found"))?;

        match key {
            Ok(k) => {
                let token: Jwk = serde_json::from_str(&k)?;
                let decoding_key = DecodingKey::from_jwk(&token)
                    .map_err(|e| anyhow::format_err!("failed to retrieve decoding key: {}", e))?;
                Ok(decoding_key)
            }
            Err(e) => Err(e),
        }
    }

    async fn get_personal_killmails(&self, user: &models::User) -> Result<(), anyhow::Error> {
        tracing::debug!(
            id = user.character_id,
            last_fetched = user
                .last_fetched
                .map_or("".to_string(), |dt| dt.to_rfc3339()),
            "fetching user killmails"
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", user.access_token).as_str().parse()?,
        );
        if let Some(last_fetched) = user.last_fetched {
            let last_modified = last_fetched.to_rfc2822();
            headers.insert("If-Modified-Since", last_modified.parse()?);
        }

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "https://esi.evetech.net/latest/characters/{}/killmails/recent/",
                user.character_id
            ))
            .headers(headers)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(anyhow::format_err!(
                "request failed with status {}: {}",
                status,
                text
            ));
        }
        let killmails: Vec<KillmailItem> = serde_json::from_str(&text)?;

        for km in killmails {
            let killmail = models::Killmail {
                killmail_id: km.killmail_id,
                killmail_hash: km.killmail_hash,
                status: "new".to_string(),
            };

            if let Err(err) = self
                .jobs_sender
                .send(processor::Job::SaveKillmail(killmail))
                .await
            {
                tracing::error!(
                    character_id = user.character_id,
                    error = err.to_string(),
                    "failed to enqueue save job"
                );
            }
        }

        Ok(())
    }

    pub async fn _get_character_info(&self, user: &models::User) -> Result<(), anyhow::Error> {
        let response = match reqwest::Client::new()
            .get(format!(
                "https://esi.evetech.net/characters/{}",
                user.character_id
            ))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return Err(anyhow::format_err!("failed to send request: {e}"));
            }
        };

        let result = match Self::json(response).await {
            Ok(json) => json,
            Err(e) => {
                return Err(anyhow::format_err!("failed to decode JSON: {e}"));
            }
        };

        let corp_id = result["corporation_id"].as_i64().unwrap_or(0);
        if corp_id != 0 {
            // self._get_corp_info(corp_id).await;
        }

        // Ok(CharacterData {
        //     name: result["name"].to_string().trim_matches('"').to_string(),
        //     corp_id: result["corporation_id"].as_i64().unwrap_or(0),
        //     alliance_id: None,
        //     updated_at: Some(chrono::Utc::now()),
        // })
        Ok(())
    }

    pub async fn _get_corp_info(&self, _corp_id: i64) {}

    async fn json(response: Response) -> Result<Value, anyhow::Error> {
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(anyhow::format_err!(
                "request failed with status {}: {}",
                status,
                text
            ));
        }
        let json: Value = serde_json::from_str(&text)?;
        Ok(json)
    }

    pub async fn refresh(&self, users: Vec<models::User>) {
        tracing::debug!(len = users.len(), "refreshing characters");
        for user in users {
            let token = Token::RefreshToken(user.refresh_token.clone());

            match self.token_exchange(token).await {
                Ok(new_user) => {
                    tracing::debug!(
                        character_id = user.character_id,
                        "refreshed token, new expiry at {}",
                        new_user.expires_at
                    );

                    if let Err(err) = self
                        .jobs_sender
                        .send(processor::Job::SaveCharacter(new_user))
                        .await
                    {
                        tracing::error!(
                            character_id = user.character_id,
                            error = err.to_string(),
                            "failed to enqueue save job"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        character_id = user.character_id,
                        error = e.to_string(),
                        "failed to refresh token"
                    );
                }
            }
        }
    }

    pub async fn get_killmails(&self, users: Vec<models::User>) {
        tracing::debug!(len = users.len(), "fetching killmails for users");
        let mut set = tokio::task::JoinSet::new();
        for user in users {
            let self_clone = self.clone();
            set.spawn(async move {
                if let Err(e) = self_clone.get_personal_killmails(&user).await {
                    tracing::error!(
                        error = e.to_string(),
                        "Failed fetch killmails for character"
                    );
                }
            });
        }
        set.join_all().await;
    }

    pub async fn resolve_killmails(&self, killmails: Vec<Killmail>) {
        tracing::debug!(len = killmails.len(), "resolving killmails");
        let mut set = tokio::task::JoinSet::new();
        for km in killmails {
            let self_clone = self.clone();
            set.spawn(async move {
                if let Err(e) = self_clone
                    .get_killmail_data(km.killmail_id, km.killmail_hash)
                    .await
                {
                    tracing::error!(
                        killmail_id = km.killmail_id,
                        error = e.to_string(),
                        "Failed to resolve killmail"
                    );
                }
            });
        }
        set.join_all().await;
    }

    pub async fn get_killmail_data(
        &self,
        killmail_id: i64,
        killmail_hash: String,
    ) -> Result<(), anyhow::Error> {
        let url = format!("https://esi.evetech.net/killmails/{killmail_id}/{killmail_hash}");
        let response = match reqwest::Client::new().get(url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(error = e.to_string(), "failed to send request");
                return Err(anyhow::format_err!("failed to send request: {e}"));
            }
        };

        let status = response.status();
        if !status.is_success() {
            tracing::error!(status = status.as_u16(), "request failed");
            return Err(anyhow::format_err!("request failed with status {}", status));
        }

        let result = match Self::json(response).await {
            Ok(json) => json,
            Err(e) => {
                tracing::error!(error = e.to_string(), "failed to decode JSON");
                return Err(anyhow::format_err!("failed to decode JSON: {e}"));
            }
        };

        let mut entities: Vec<models::Entity> = Vec::new();

        let solar_system: models::Entity;
        if let Some(system_id) = result.get("solar_system_id").and_then(|id| id.as_i64()) {
            solar_system = models::Entity {
                id: system_id,
                name: "".to_string(),
                type_: "solar_system".to_string(),
            };
        } else {
            tracing::error!("killmail missing solar_system_id");
            solar_system = models::Entity {
                id: 0,
                name: "".to_string(),
                type_: "solar_system".to_string(),
            };
        }
        entities.push(solar_system);
        if let Some(victim) = result.get("victim") {
            if let Some(char_id) = victim.get("character_id").and_then(|id| id.as_i64()) {
                entities.push(models::Entity {
                    id: char_id,
                    name: "".to_string(),
                    type_: "character".to_string(),
                });
            }
            if let Some(corp_id) = victim.get("corporation_id").and_then(|id| id.as_i64()) {
                entities.push(models::Entity {
                    id: corp_id,
                    name: "".to_string(),
                    type_: "corporation".to_string(),
                });
            }
            if let Some(alliance_id) = victim.get("alliance_id").and_then(|id| id.as_i64()) {
                entities.push(models::Entity {
                    id: alliance_id,
                    name: "".to_string(),
                    type_: "alliance".to_string(),
                });
            }
            if let Some(weapon_type_id) = victim.get("weapon_type_id").and_then(|id| id.as_i64()) {
                entities.push(models::Entity {
                    id: weapon_type_id,
                    name: "".to_string(),
                    type_: "weapon_type".to_string(),
                });
            }
            if let Some(ship_type_id) = victim.get("ship_type_id").and_then(|id| id.as_i64()) {
                entities.push(models::Entity {
                    id: ship_type_id,
                    name: "".to_string(),
                    type_: "ship_type".to_string(),
                });
            }
        }

        if let Some(attackers) = result.get("attackers").and_then(|a| a.as_array()) {
            for attacker in attackers {
                if let Some(char_id) = attacker.get("character_id").and_then(|id| id.as_i64()) {
                    entities.push(models::Entity {
                        id: char_id,
                        name: "".to_string(),
                        type_: "character".to_string(),
                    });
                }
                if let Some(corp_id) = attacker.get("corporation_id").and_then(|id| id.as_i64()) {
                    entities.push(models::Entity {
                        id: corp_id,
                        name: "".to_string(),
                        type_: "corporation".to_string(),
                    });
                }
                if let Some(alliance_id) = attacker.get("alliance_id").and_then(|id| id.as_i64()) {
                    entities.push(models::Entity {
                        id: alliance_id,
                        name: "".to_string(),
                        type_: "alliance".to_string(),
                    });
                }
                if let Some(ship_type_id) = attacker.get("ship_type_id").and_then(|id| id.as_i64())
                {
                    entities.push(models::Entity {
                        id: ship_type_id,
                        name: "".to_string(),
                        type_: "ship_type".to_string(),
                    });
                }
            }
        }

        tracing::trace!(
            killmail_id,
            len = entities.len(),
            "entities collected from killmail"
        );
        for entity in entities {
            tracing::trace!(entity = format!("{entity:?}"), "debugging entity");
            if entity.id == 0 {
                continue;
            }
            if let Err(err) = self
                .jobs_sender
                .send(processor::Job::SaveEntity(entity))
                .await
            {
                tracing::error!(
                    killmail_id,
                    error = err.to_string(),
                    "failed to enqueue save job"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    #[tokio::test]
    async fn test_parse_last_modified_value() {
        let last_modified = "Tue, 30 Sep 2025 22:55:17 GMT".to_string();
        let parsed = chrono::DateTime::parse_from_rfc2822(&last_modified).unwrap();
        println!("{}", parsed.to_utc().to_rfc3339());
    }
}
