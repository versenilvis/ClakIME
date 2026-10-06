use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentStatus {
    pub gtk_im_module: Option<String>,
    pub qt_im_module: Option<String>,
    pub xmodifiers: Option<String>,
    pub env_file_exists: bool,
    pub env_file_path: PathBuf,
}

// inspect input method environment variables and configuration file
pub fn check_environment() -> EnvironmentStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let env_file_path = PathBuf::from(home).join(".config/environment.d/99-clak-im.conf");
    let env_file_exists = env_file_path.exists();

    EnvironmentStatus {
        gtk_im_module: env::var("GTK_IM_MODULE").ok(),
        qt_im_module: env::var("QT_IM_MODULE").ok(),
        xmodifiers: env::var("XMODIFIERS").ok(),
        env_file_exists,
        env_file_path,
    }
}
