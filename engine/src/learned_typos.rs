use crate::{english_dict, spelling};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearnedFile {
    #[serde(default)]
    pub typos: Vec<LearnedEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedEntry {
    pub raw: String,
    pub target: String,
    #[serde(default = "default_count")]
    pub count: usize,
}

fn default_count() -> usize {
    2
}

// calculates standard levenshtein edit distance between two strings
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let m = v1.len();
    let n = v2.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut dp = vec![vec![0; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if v1[i - 1] == v2[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[m][n]
}

pub struct LearnedTypos {
    typos: HashMap<String, String>,
    frequencies: HashMap<(String, String), usize>,
    config_path: Option<PathBuf>,
}

impl Default for LearnedTypos {
    fn default() -> Self {
        Self::new()
    }
}

impl LearnedTypos {
    pub fn new() -> Self {
        let config_path = std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config/clak/learned_typos.toml"));
        let mut inst = Self {
            typos: HashMap::new(),
            frequencies: HashMap::new(),
            config_path,
        };
        inst.load();
        inst
    }

    pub fn with_path(path: Option<PathBuf>) -> Self {
        let mut inst = Self {
            typos: HashMap::new(),
            frequencies: HashMap::new(),
            config_path: path,
        };
        inst.load();
        inst
    }

    pub fn load(&mut self) {
        let path = match &self.config_path {
            Some(p) => p,
            None => return,
        };
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(parsed) = toml::from_str::<LearnedFile>(&content) {
                for entry in parsed.typos {
                    self.typos
                        .insert(entry.raw.to_lowercase(), entry.target.clone());
                    self.frequencies
                        .insert((entry.raw.to_lowercase(), entry.target), entry.count);
                }
            }
        }
    }

    pub fn save(&self) {
        let path = match &self.config_path {
            Some(p) => p,
            None => return,
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let entries: Vec<LearnedEntry> = self
            .typos
            .iter()
            .map(|(raw, target)| {
                let count = self
                    .frequencies
                    .get(&(raw.clone(), target.clone()))
                    .copied()
                    .unwrap_or(2);
                LearnedEntry {
                    raw: raw.clone(),
                    target: target.clone(),
                    count,
                }
            })
            .collect();
        let file_obj = LearnedFile { typos: entries };
        if let Ok(toml_str) = toml::to_string_pretty(&file_obj) {
            let _ = fs::write(path, toml_str);
        }
    }

    pub fn lookup(&self, raw_lower: &str) -> Option<&String> {
        self.typos.get(raw_lower)
    }

    pub fn observe(&mut self, old_raw: &str, new_word: &str) -> Option<String> {
        let raw_lower = old_raw.trim().to_lowercase();
        let target = new_word.trim().to_string();
        let target_lower = target.to_lowercase();

        if raw_lower.len() < 2 || raw_lower.len() > 10 || target_lower.len() < 2 {
            return None;
        }

        // guard 1: old raw must be purely alphabetic
        if !raw_lower.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }

        // guard 2: old raw must not be a known english word
        if english_dict::is_english(&raw_lower) {
            return None;
        }

        // guard 3: old raw must not already be a valid vietnamese word
        if crate::dict().binary_search(&raw_lower.as_str()).is_ok() {
            return None;
        }

        // guard 4: target must be a valid vietnamese word in dictionary
        let cvc_ok = spelling::is_valid_cvc(&target);
        let dict_ok = crate::dict().binary_search(&target_lower.as_str()).is_ok();
        if !cvc_ok || !dict_ok {
            return None;
        }

        if raw_lower == target_lower {
            return None;
        }

        // guard 5: similarity check
        let decomp = crate::ime::decompose_to_telex(&target).to_lowercase();
        let dist1 = levenshtein_distance(&raw_lower, &decomp);
        let dist2 = levenshtein_distance(&raw_lower, &target_lower);
        let min_dist = dist1.min(dist2);
        let max_len = raw_lower.chars().count().max(decomp.chars().count());

        // edit distance threshold
        if min_dist > 2 && (max_len < 6 || min_dist > 3) {
            return None;
        }

        // guard 6: frequency threshold (must be seen at least 2 times)
        let key = (raw_lower.clone(), target.clone());
        let count = self.frequencies.entry(key).or_insert(0);
        *count += 1;

        if *count >= 2 {
            self.typos.insert(raw_lower, target.clone());
            self.save();
            return Some(target);
        }

        None
    }
}

static LEARNED: OnceLock<RwLock<LearnedTypos>> = OnceLock::new();

pub fn instance() -> &'static RwLock<LearnedTypos> {
    LEARNED.get_or_init(|| RwLock::new(LearnedTypos::new()))
}

pub fn lookup_learned_typo(raw_lower: &str) -> Option<String> {
    instance().read().ok()?.lookup(raw_lower).cloned()
}

pub fn observe_retype(old_raw: &str, new_word: &str) -> Option<String> {
    instance().write().ok()?.observe(old_raw, new_word)
}

pub fn clear_for_test() {
    if let Ok(mut lock) = instance().write() {
        lock.typos.clear();
        lock.frequencies.clear();
    }
}
