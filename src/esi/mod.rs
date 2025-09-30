use std::collections::HashMap;

use crate::{User, config::Config};
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode, jwk::Jwk};
use reqwest::Response;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub mod processor;

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
}

#[derive(Clone, Debug, Deserialize)]
struct KillmailItem {
    #[serde(rename = "killmail_hash")]
    _killmail_hash: String,
    #[serde(rename = "killmail_id")]
    _killmail_id: i64,
}

impl EsiClient {
    pub fn from_config(config: Config) -> Self {
        Self {
            app_id: config.application_id,
            app_secret: config.application_secret,
            redirect_uri: config.redirect_uri,
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

    pub async fn token_exchange(&self, token: Token) -> Result<User, anyhow::Error> {
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
                let user = User::new(access_token, refresh_token, claims);
                tracing::debug!(
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
        tracing::debug!(token, "attempting to validate token");
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

    pub async fn _get_personal_killmails(&self, user: &User) -> Result<(), anyhow::Error> {
        tracing::info!(
            id = user.character_id,
            access_token = user.access_token,
            "fetching user killmails"
        );
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "https://esi.evetech.net/latest/characters/{}/killmails/recent/",
                user.character_id
            ))
            .header("Authorization", format!("Bearer {}", user.access_token))
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
        let _: Vec<KillmailItem> = serde_json::from_str(&text)?;
        // let mut km = Killmails::new();

        Ok(())
    }

    pub async fn _get_character_info(&self, user: &User) -> Result<(), anyhow::Error> {
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

    pub async fn refresh(
        &self,
        users: HashMap<i64, User>,
    ) -> Result<HashMap<i64, User>, anyhow::Error> {
        if users.is_empty() {
            tracing::info!("no users to refresh");
            return Ok(HashMap::new());
        }

        let users_to_refresh: Vec<User> = {
            tracing::info!(users = format!("{:?}", users), "refreshing characters");
            users.values().cloned().collect()
        };

        let mut refreshed_users = HashMap::new();

        for user in users_to_refresh {
            let token = Token::RefreshToken(user.refresh_token.clone());

            match self.token_exchange(token).await {
                Ok(new_user) => {
                    tracing::info!(
                        character_id = user.character_id,
                        "refreshed token, new expiry at {}",
                        new_user.expires_at
                    );
                    refreshed_users.insert(new_user.character_id, new_user);
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

        Ok(refreshed_users)
    }
}
