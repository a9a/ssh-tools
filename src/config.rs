use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const CONFIG_ENV_KEY: &str = "SSH_QC_CONFIG";

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# ssh-quick-connect configuration
# Add as many machines as you need.

[[connections]]
id = "dev"
name = "dev-server"
host = "192.168.1.10"
user = "dev"
port = 22
identity_file = "~/.ssh/id_ed25519"
options = ["StrictHostKeyChecking=accept-new"]

[[connections]]
id = "prod"
name = "prod-jump"
host = "jump.example.com"
user = "ops"
options = ["ServerAliveInterval=30", "ServerAliveCountMax=3"]
"#;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub connections: Vec<Connection>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct Connection {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<PathBuf>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
    Validation(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::ParseToml(err) => write!(f, "Invalid TOML format: {err}"),
            Self::Validation(msg) => write!(f, "Invalid configuration: {msg}"),
        }
    }
}

impl Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::ParseToml(value)
    }
}

pub fn resolve_config_path() -> PathBuf {
    if let Ok(path) = env::var(CONFIG_ENV_KEY) {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = env::var("APPDATA") {
            return PathBuf::from(app_data)
                .join("ssh-quick-connect")
                .join("config.toml");
        }
        if let Ok(user_profile) = env::var("USERPROFILE") {
            return PathBuf::from(user_profile)
                .join(".ssh-quick-connect")
                .join("config.toml");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg_config_home)
                .join("ssh-quick-connect")
                .join("config.toml");
        }
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("ssh-quick-connect")
                .join("config.toml");
        }
    }

    PathBuf::from("config.toml")
}

pub fn ensure_config_exists(path: &Path) -> Result<bool, ConfigError> {
    if path.exists() {
        return Ok(false);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
    Ok(true)
}

pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    let raw = fs::read_to_string(path)?;
    let parsed: AppConfig = toml::from_str(&raw)?;
    validate_config(&parsed)?;
    Ok(parsed)
}

pub fn validate_config(config: &AppConfig) -> Result<(), ConfigError> {
    if config.connections.is_empty() {
        return Err(ConfigError::Validation(
            "connections list must contain at least one entry".to_string(),
        ));
    }

    let mut seen = HashSet::new();
    let mut seen_ids = HashSet::new();
    for connection in &config.connections {
        if let Some(id) = &connection.id {
            if id.trim().is_empty() {
                return Err(ConfigError::Validation(
                    "connection id cannot be empty".to_string(),
                ));
            }
            if !seen_ids.insert(id.to_lowercase()) {
                return Err(ConfigError::Validation(format!(
                    "duplicate connection id '{}'",
                    id
                )));
            }
        }
        if connection.name.trim().is_empty() {
            return Err(ConfigError::Validation(
                "connection name cannot be empty".to_string(),
            ));
        }
        if connection.host.trim().is_empty() {
            return Err(ConfigError::Validation(format!(
                "connection '{}' has empty host",
                connection.name
            )));
        }
        if !seen.insert(connection.name.to_lowercase()) {
            return Err(ConfigError::Validation(format!(
                "duplicate connection name '{}'",
                connection.name
            )));
        }
        if connection.options.iter().any(|opt| opt.trim().is_empty()) {
            return Err(ConfigError::Validation(format!(
                "connection '{}' contains an empty ssh option",
                connection.name
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, Connection, validate_config};
    use std::path::PathBuf;

    #[test]
    fn validates_valid_config() {
        let config = AppConfig {
            connections: vec![Connection {
                id: Some("srv".to_string()),
                name: "srv".to_string(),
                host: "example.com".to_string(),
                user: Some("alice".to_string()),
                port: Some(2222),
                identity_file: Some(PathBuf::from("~/.ssh/id_ed25519")),
                options: vec!["StrictHostKeyChecking=accept-new".to_string()],
            }],
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn rejects_duplicate_names() {
        let config = AppConfig {
            connections: vec![
                Connection {
                    id: Some("srv-a".to_string()),
                    name: "srv".to_string(),
                    host: "a.example.com".to_string(),
                    user: None,
                    port: None,
                    identity_file: None,
                    options: vec![],
                },
                Connection {
                    id: Some("srv-b".to_string()),
                    name: "SRV".to_string(),
                    host: "b.example.com".to_string(),
                    user: None,
                    port: None,
                    identity_file: None,
                    options: vec![],
                },
            ],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn rejects_empty_host() {
        let config = AppConfig {
            connections: vec![Connection {
                id: Some("srv".to_string()),
                name: "srv".to_string(),
                host: "   ".to_string(),
                user: None,
                port: None,
                identity_file: None,
                options: vec![],
            }],
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let config = AppConfig {
            connections: vec![
                Connection {
                    id: Some("prod".to_string()),
                    name: "srv-a".to_string(),
                    host: "a.example.com".to_string(),
                    user: None,
                    port: None,
                    identity_file: None,
                    options: vec![],
                },
                Connection {
                    id: Some("PROD".to_string()),
                    name: "srv-b".to_string(),
                    host: "b.example.com".to_string(),
                    user: None,
                    port: None,
                    identity_file: None,
                    options: vec![],
                },
            ],
        };

        assert!(validate_config(&config).is_err());
    }
}
