use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,

    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: u64,

    #[serde(default = "default_max_entries")]
    pub max_entries: u64,

    #[serde(default = "default_op_path")]
    pub op_path: String,

    #[serde(default = "default_op_timeout_seconds")]
    pub op_timeout_seconds: u64,
}

fn default_socket_path() -> PathBuf {
    // Prefer a user-owned runtime directory over world-writable /tmp.
    // On Linux, XDG_RUNTIME_DIR (/run/user/UID/) is only writable by the
    // owning user, preventing rogue socket pre-creation attacks.
    if let Some(base) = directories::BaseDirs::new() {
        if let Some(runtime_dir) = base.runtime_dir() {
            // Linux: /run/user/UID/ — guaranteed user-exclusive
            return runtime_dir.join("op-cache.sock");
        }
        // macOS / other: use user cache dir (user-owned, not world-writable)
        if let Some(proj) = directories::ProjectDirs::from("", "", "op-cache") {
            return proj.cache_dir().join("op-cache.sock");
        }
    }
    // Final fallback (e.g. no $HOME set)
    PathBuf::from("/tmp/op-cache.sock")
}

fn default_ttl_seconds() -> u64 {
    // 1 hour: balances performance (avoids repeated op read calls) with
    // the risk of serving revoked credentials after rotation. Operators
    // with stricter rotation policies should set a lower value in config.
    3600
}

fn default_max_entries() -> u64 {
    1000
}

fn default_op_path() -> String {
    "op".to_string()
}

fn default_op_timeout_seconds() -> u64 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            ttl_seconds: default_ttl_seconds(),
            max_entries: default_max_entries(),
            op_path: default_op_path(),
            op_timeout_seconds: default_op_timeout_seconds(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_yaml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "op-cache") {
            Ok(proj_dirs.config_dir().join("config.yaml"))
        } else {
            Ok(PathBuf::from("~/.config/op-cache/config.yaml"))
        }
    }

    pub fn pid_path(&self) -> PathBuf {
        self.socket_path.with_extension("pid")
    }

    pub fn log_path(&self) -> PathBuf {
        self.socket_path.with_extension("log")
    }
}
