use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStatus {
    pub config_file_exists: bool,
    pub config_path: PathBuf,
    pub log_file_exists: bool,
    pub log_path: PathBuf,
    pub log_size_kb: Option<f64>,
}

pub fn check_config() -> ConfigStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_path = PathBuf::from(&home).join(".config/clak/config.toml");
    let config_file_exists = config_path.exists();

    let log_path = PathBuf::from("/tmp/clak.log");
    let log_file_exists = log_path.exists();
    let log_size_kb = if log_file_exists {
        fs::metadata(&log_path)
            .ok()
            .map(|m| m.len() as f64 / 1024.0)
    } else {
        None
    };

    ConfigStatus {
        config_file_exists,
        config_path,
        log_file_exists,
        log_path,
        log_size_kb,
    }
}
