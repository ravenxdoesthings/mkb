use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub application_id: String,
    pub application_secret: String,
    pub redirect_uri: String,
    pub database_uri: String,
}

impl Config {
    pub fn from_env() -> Self {
        let application_id = std::env::var("MKB_ESI_APPLICATION_ID")
            .expect("MKB_ESI_APPLICATION_ID environment variable not set");
        let application_secret = std::env::var("MKB_ESI_APPLICATION_SECRET")
            .expect("MKB_ESI_APPLICATION_SECRET environment variable not set");
        let redirect_uri = std::env::var("MKB_ESI_REDIRECT_URI")
            .expect("MKB_ESI_REDIRECT_URI environment variable not set");
        let database_uri =
            std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable not set");

        Self {
            application_id,
            application_secret,
            redirect_uri,
            database_uri,
        }
    }

    pub fn _from_file() -> Result<Self, anyhow::Error> {
        let path = std::env::var("MKB_CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());
        let file_content = std::fs::read_to_string(path).expect("Failed to read config file");
        match serde_json::from_str(&file_content) {
            Ok(config) => Ok(config),
            _ => Err(anyhow::format_err!("Failed to parse config file")),
        }
    }
}
