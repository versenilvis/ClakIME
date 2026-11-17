pub fn check_can_rollback() -> (bool, bool, String, Option<String>) {
    let home = match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => h,
        _ => return (false, false, String::new(), None),
    };

    // check aur or pacman cache if installed via pacman
    let is_pacman = std::process::Command::new("pacman")
        .args(["-Qi", "clak"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || std::process::Command::new("pacman")
            .args(["-Qi", "clak-bin"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

    if is_pacman {
        let cache_dirs = [
            std::path::PathBuf::from("/var/cache/pacman/pkg"),
            std::path::Path::new(&home).join(".cache/yay/clak"),
            std::path::Path::new(&home).join(".cache/yay/clak-bin"),
            std::path::Path::new(&home).join(".cache/paru/clone/clak"),
            std::path::Path::new(&home).join(".cache/paru/clone/clak-bin"),
        ];

        // get current installed pacman version
        let cur_pkg_ver = std::process::Command::new("pacman")
            .args(["-Q", "clak-bin"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout).ok()
                } else {
                    None
                }
            })
            .or_else(|| {
                std::process::Command::new("pacman")
                    .args(["-Q", "clak"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                        } else {
                            None
                        }
                    })
            })
            .and_then(|out| {
                out.split_whitespace()
                    .nth(1)
                    .map(|v| v.split('-').next().unwrap_or("").to_string())
            })
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

        let mut candidates: Vec<String> = Vec::new();
        for cd in &cache_dirs {
            if let Ok(entries) = std::fs::read_dir(cd) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.contains(".pkg.tar.") {
                        let prefixes = ["clak-bin-", "clak-git-", "clak-"];
                        for prefix in prefixes {
                            if let Some(rest) = fname.strip_prefix(prefix) {
                                if let Some(dash_idx) = rest.find('-') {
                                    let v_str = &rest[..dash_idx];
                                    if v_str.chars().all(|c| c.is_ascii_digit() || c == '.')
                                        && v_str.contains('.')
                                    {
                                        candidates.push(v_str.to_string());
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        // sort semver descending and find latest version strictly less than current
        let parse_semver = |v: &str| -> (u32, u32, u32) {
            let mut parts = v.split('.').filter_map(|s| s.parse::<u32>().ok());
            (
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
            )
        };

        candidates.sort_by_key(|b| std::cmp::Reverse(parse_semver(b)));
        candidates.dedup();

        let cur_parsed = parse_semver(&cur_pkg_ver);
        for cand in candidates {
            if parse_semver(&cand) < cur_parsed {
                return (
                    true,
                    true,
                    format!("Bản trước trong cache: v{}", cand),
                    None,
                );
            }
        }
        return (
            true,
            false,
            "Không có gói cũ hơn trong cache pacman/AUR".to_string(),
            None,
        );
    }

    // check local backup
    let backup_dir = std::path::Path::new(&home).join(".local/share/clak/backup");
    let version_prev_file = backup_dir.join("version_prev");
    let lib_so = backup_dir.join("libclak.so");
    if lib_so.exists() && version_prev_file.exists() {
        if let Ok(prev_ver) = std::fs::read_to_string(&version_prev_file) {
            let ver = prev_ver.trim();
            if !ver.is_empty() {
                return (
                    false,
                    true,
                    format!("Bản trước: {}", ver),
                    Some(ver.to_string()),
                );
            }
        }
    }

    // check standalone version_prev file
    let standalone_prev = std::path::Path::new(&home).join(".local/share/clak/version_prev");
    if standalone_prev.exists() {
        if let Ok(prev_ver) = std::fs::read_to_string(&standalone_prev) {
            let ver = prev_ver.trim();
            if !ver.is_empty() {
                return (
                    false,
                    false,
                    format!("Bản trước: {}", ver),
                    Some(ver.to_string()),
                );
            }
        }
    }

    (false, false, String::new(), None)
}

#[derive(Clone, Debug)]
pub struct VersionItem {
    pub label: String,
    pub target: String,
    pub is_backup: bool,
    pub is_pacman_pkg: bool,
}

pub fn fetch_available_versions(
    home: &str,
    is_pacman: bool,
    prev_backup_ver: Option<&str>,
) -> Vec<VersionItem> {
    let cur_pkg_ver = env!("CARGO_PKG_VERSION");

    let mut items = vec![VersionItem {
        label: "Mới nhất (Hiện tại)".to_string(),
        target: String::new(),
        is_backup: false,
        is_pacman_pkg: false,
    }];

    let parse_semver = |v: &str| -> (u32, u32, u32) {
        let trimmed = v.trim_start_matches('v');
        let mut parts = trimmed.split('.').filter_map(|s| s.parse::<u32>().ok());
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    };
    let cur_parsed = parse_semver(cur_pkg_ver);

    if is_pacman {
        let cache_dirs = [
            std::path::PathBuf::from("/var/cache/pacman/pkg"),
            std::path::Path::new(home).join(".cache/yay/clak"),
            std::path::Path::new(home).join(".cache/yay/clak-bin"),
            std::path::Path::new(home).join(".cache/paru/clone/clak"),
            std::path::Path::new(home).join(".cache/paru/clone/clak-bin"),
        ];

        let mut found: Vec<((u32, u32, u32), String, String)> = Vec::new();
        for cd in &cache_dirs {
            if let Ok(entries) = std::fs::read_dir(cd) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().to_string();
                    if fname.contains(".pkg.tar.") {
                        let prefixes = ["clak-bin-", "clak-git-", "clak-"];
                        for prefix in prefixes {
                            if let Some(rest) = fname.strip_prefix(prefix) {
                                if let Some(dash_idx) = rest.find('-') {
                                    let v_str = &rest[..dash_idx];
                                    let sem = parse_semver(v_str);
                                    if sem < cur_parsed && sem >= (0, 7, 0) {
                                        found.push((
                                            sem,
                                            format!("v{}", v_str),
                                            entry.path().to_string_lossy().to_string(),
                                        ));
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        found.sort_by(|a, b| b.0.cmp(&a.0));
        let mut seen = std::collections::HashSet::new();
        for (_, v_str, path) in found {
            if seen.insert(v_str.clone()) {
                items.push(VersionItem {
                    label: format!("{} (Trong cache)", v_str),
                    target: path,
                    is_backup: false,
                    is_pacman_pkg: true,
                });
            }
        }
        return items;
    }

    // script install branch: github releases + local backup
    let cache_dir = std::path::Path::new(home).join(".local/share/clak");
    let cache_file = cache_dir.join("releases_cache.json");
    let mut json_str = String::new();

    let is_cache_valid = if let Ok(meta) = std::fs::metadata(&cache_file) {
        if let Ok(mtime) = meta.modified() {
            if let Ok(elapsed) = mtime.elapsed() {
                elapsed.as_secs() < 3600
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if is_cache_valid {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            json_str = content;
        }
    }

    if json_str.is_empty() {
        let output = std::process::Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "4",
                "-H",
                "Accept: application/vnd.github.v3+json",
                "-H",
                "User-Agent: clak-gui",
                "https://api.github.com/repos/versenilvis/ClakIME/releases",
            ])
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if !s.trim().is_empty() {
                        let _ = std::fs::create_dir_all(&cache_dir);
                        let _ = std::fs::write(&cache_file, &s);
                        json_str = s;
                    }
                }
            }
        }
    }

    // fallback to stale cache if it exists
    if json_str.is_empty() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            json_str = content;
        }
    }

    let min_semver = (0, 8, 0);
    let cur_tag = format!("v{}", cur_pkg_ver);

    if let Ok(releases) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
        for r in releases {
            let is_draft = r.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_prerelease = r
                .get("prerelease")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_draft || is_prerelease {
                continue;
            }

            let tag = r.get("tag_name").and_then(|v| v.as_str()).unwrap_or("");
            if tag.is_empty() || tag == cur_tag {
                continue;
            }

            if parse_semver(tag) < min_semver {
                continue;
            }

            let has_tarball = r
                .get("assets")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter().any(|a| {
                        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        name.contains("x86_64") && name.ends_with(".tar.gz")
                    })
                })
                .unwrap_or(false);

            if !has_tarball {
                continue;
            }

            let is_backup = Some(tag) == prev_backup_ver;
            let label = if is_backup {
                format!("{} (Bản sao lưu)", tag)
            } else {
                tag.to_string()
            };

            items.push(VersionItem {
                label,
                target: tag.to_string(),
                is_backup,
                is_pacman_pkg: false,
            });

            if items.len() >= 8 {
                break;
            }
        }
    }

    if items.len() == 1 {
        if let Some(prev) = prev_backup_ver {
            if prev != cur_tag {
                items.push(VersionItem {
                    label: format!("{} (Bản sao lưu)", prev),
                    target: prev.to_string(),
                    is_backup: true,
                    is_pacman_pkg: false,
                });
            }
        }
    }

    items
}

