use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentStatus {
    pub gtk_im_module: Option<String>,
    pub qt_im_module: Option<String>,
    pub xmodifiers: Option<String>,
    #[serde(default)]
    pub sdl_im_module: Option<String>,
    #[serde(default)]
    pub glfw_im_module: Option<String>,
    pub env_file_exists: bool,
    pub env_file_path: PathBuf,
}

// inspect input method environment variables from a provided lookup
pub fn check_environment_from<F>(
    get_var: F,
    env_file_exists: bool,
    env_file_path: PathBuf,
) -> EnvironmentStatus
where
    F: Fn(&str) -> Option<String>,
{
    EnvironmentStatus {
        gtk_im_module: get_var("GTK_IM_MODULE"),
        qt_im_module: get_var("QT_IM_MODULE"),
        xmodifiers: get_var("XMODIFIERS"),
        sdl_im_module: get_var("SDL_IM_MODULE"),
        glfw_im_module: get_var("GLFW_IM_MODULE"),
        env_file_exists,
        env_file_path,
    }
}

// inspect input method environment variables and configuration file
pub fn check_environment() -> EnvironmentStatus {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let env_file_path = PathBuf::from(home).join(".config/environment.d/99-clak-im.conf");
    let env_file_exists = env_file_path.exists();

    check_environment_from(|k| env::var(k).ok(), env_file_exists, env_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_environment_captures_sdl_and_glfw_modules() {
        let status = check_environment_from(
            |k| match k {
                "GTK_IM_MODULE" => Some("fcitx".to_string()),
                "QT_IM_MODULE" => Some("fcitx".to_string()),
                "XMODIFIERS" => Some("@im=fcitx".to_string()),
                "SDL_IM_MODULE" => Some("ibus".to_string()),
                "GLFW_IM_MODULE" => Some("ibus".to_string()),
                _ => None,
            },
            true,
            PathBuf::from("/tmp/99-clak-im.conf"),
        );
        assert_eq!(status.sdl_im_module.as_deref(), Some("ibus"));
        assert_eq!(status.glfw_im_module.as_deref(), Some("ibus"));
        assert!(status.env_file_exists);
    }
}
