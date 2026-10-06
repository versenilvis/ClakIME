use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInspection {
    pub clak_loaded: bool,
    pub is_deleted_inode: bool,
    pub mapped_path: Option<String>,
}

pub fn check_memory(pids: &[u32]) -> MemoryInspection {
    let mut clak_loaded = false;
    let mut is_deleted_inode = false;
    let mut mapped_path = None;

    if let Some(&pid) = pids.first() {
        let maps_path = format!("/proc/{}/maps", pid);
        if let Ok(content) = fs::read_to_string(maps_path) {
            for line in content.lines() {
                if line.contains("clak") {
                    clak_loaded = true;
                    if line.contains("(deleted)") {
                        is_deleted_inode = true;
                    }
                    if mapped_path.is_none() {
                        // find path token in /proc/pid/maps line
                        let tokens: Vec<&str> = line.split_whitespace().collect();
                        for token in tokens {
                            if token.starts_with('/') && token.contains("clak") {
                                mapped_path = Some(token.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    MemoryInspection {
        clak_loaded,
        is_deleted_inode,
        mapped_path,
    }
}
