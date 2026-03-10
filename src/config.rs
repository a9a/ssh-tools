use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io::Write;
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
options = []

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

    write_default_config(path)?;
    Ok(true)
}

pub fn load_config(path: &Path) -> Result<AppConfig, ConfigError> {
    verify_config_security(path)?;
    let raw = fs::read_to_string(path)?;
    let parsed: AppConfig = toml::from_str(&raw)?;
    validate_config(&parsed)?;
    Ok(parsed)
}

fn write_default_config(path: &Path) -> Result<(), ConfigError> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(DEFAULT_CONFIG_TEMPLATE.as_bytes())?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
        Ok(())
    }
}

fn verify_config_security(path: &Path) -> Result<(), ConfigError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(ConfigError::Validation(format!(
            "config path '{}' must point to a regular file",
            path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        let mode = metadata.mode();
        if mode & 0o022 != 0 {
            return Err(ConfigError::Validation(format!(
                "config '{}' is writable by group/others; run chmod 600",
                path.display()
            )));
        }

        let uid = metadata.uid();
        // SAFETY: `geteuid` has no preconditions and returns effective user id of current process.
        let euid = unsafe { libc::geteuid() };

        if uid != euid && uid != 0 {
            return Err(ConfigError::Validation(format!(
                "config '{}' must be owned by the current user (or root)",
                path.display()
            )));
        }
    }

    Ok(())
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
    use super::{AppConfig, Connection, ensure_config_exists, load_config, validate_config};
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

    #[cfg(unix)]
    #[test]
    fn rejects_world_writable_config() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssh-qc-test-{unique}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            r#"[[connections]]
id = "dev"
name = "dev"
host = "localhost"
"#,
        )
        .expect("failed to write config");
        fs::set_permissions(&config_path, fs::Permissions::from_mode(0o666))
            .expect("failed to set file perms");

        let result = load_config(&config_path);
        assert!(result.is_err());

        let _ = fs::remove_file(&config_path);
        let _ = fs::remove_dir(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn creates_default_config_with_0600_permissions() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ssh-qc-create-{unique}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        let config_path = dir.join("config.toml");

        let created = ensure_config_exists(&config_path).expect("expected config to be created");
        assert!(created);

        let mode = fs::metadata(&config_path)
            .expect("metadata should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        let _ = fs::remove_file(&config_path);
        let _ = fs::remove_dir(&dir);
    }
}
