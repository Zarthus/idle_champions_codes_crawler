use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub dry_run: bool,

    pub client: ClientConfig,

    pub discord: HashMap<String, DiscordConfig>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ClientConfig {
    pub remote_host: Option<String>,
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DiscordConfig {
    /// Enabled: Required
    pub enabled: bool,
    /// Send acknowledgements (reactions) to cache remotely and display the bot handled it to others;
    /// This increases the number of requests to discord by 1 for each message parsed (only the first time)
    pub acknowledge: bool,
    /// Application ID: Optional, improved logging
    pub application_id: u64,
    /// Public Key: Optional
    pub public_key: String,
    /// Bot Token: Required - HTTP request auth
    pub bot_token: String,
    /// Guild ID: Optional (but fallback for good url generation)
    pub guild_id: u64,
    /// Channel ID: Required - which channel to read
    pub channel_id: u64,
}

pub fn dir() -> PathBuf {
    directories::ProjectDirs::from("net", "liefland", "liccrawler")
        .unwrap()
        .config_dir()
        .to_path_buf()
}

fn setup() {
    let config_dir = dir();
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir).unwrap();

        write(Config::default());
    }
}

pub fn write(config: Config) {
    setup();

    std::fs::write(dir().join("config.toml"), toml::to_string(&config).unwrap()).unwrap();
}

pub fn read() -> Config {
    setup();

    let cfg = std::fs::read_to_string(dir().join("config.toml")).unwrap();

    let config: Config = toml::from_str(&cfg).unwrap();

    config
}

impl Default for Config {
    fn default() -> Self {
        let mut d: HashMap<String, DiscordConfig> = HashMap::new();
        d.insert("default".to_string(), DiscordConfig::default());

        Self {
            dry_run: false,
            client: ClientConfig::default(),
            discord: d,
        }
    }
}
