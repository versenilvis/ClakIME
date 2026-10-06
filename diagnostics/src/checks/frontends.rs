use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendStatus {
    pub gtk3: bool,
    pub gtk4: bool,
    pub qt5: bool,
    pub qt6: bool,
    pub gtk3_path: Option<String>,
    pub gtk4_path: Option<String>,
    pub qt5_path: Option<String>,
    pub qt6_path: Option<String>,
}

fn find_file_under(prefixes: &[&str], subpaths: &[&str]) -> Option<String> {
    for prefix in prefixes {
        for sub in subpaths {
            let p = Path::new(prefix).join(sub);
            if p.exists() {
                return Some(p.display().to_string());
            }
        }
    }
    None
}

// inspect presence of fcitx5 input method frontend libraries
pub fn check_frontends() -> FrontendStatus {
    let prefixes = [
        "/usr/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/local/lib",
    ];

    let gtk3_sub = [
        "gtk-3.0/3.0.0/immodules/im-fcitx5.so",
        "gtk-3.0/3.0.0/immodules/libim-fcitx5.so",
    ];
    let gtk4_sub = [
        "gtk-4.0/4.0.0/immodules/libim-fcitx5.so",
        "gtk-4.0/4.0.0/immodules/im-fcitx5.so",
    ];
    let qt5_sub = [
        "qt/plugins/platforminputcontexts/libfcitx5platforminputcontextplugin.so",
        "qt5/plugins/platforminputcontexts/libfcitx5platforminputcontextplugin.so",
    ];
    let qt6_sub = [
        "qt6/plugins/platforminputcontexts/libfcitx5platforminputcontextplugin.so",
    ];

    let gtk3_path = find_file_under(&prefixes, &gtk3_sub);
    let gtk4_path = find_file_under(&prefixes, &gtk4_sub);
    let qt5_path = find_file_under(&prefixes, &qt5_sub);
    let qt6_path = find_file_under(&prefixes, &qt6_sub);

    FrontendStatus {
        gtk3: gtk3_path.is_some(),
        gtk4: gtk4_path.is_some(),
        qt5: qt5_path.is_some(),
        qt6: qt6_path.is_some(),
        gtk3_path,
        gtk4_path,
        qt5_path,
        qt6_path,
    }
}
