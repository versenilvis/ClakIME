use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

pub struct Percentiles {
    pub count: usize,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
}

pub fn calculate_percentiles(mut values: Vec<f64>) -> Option<Percentiles> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();

    let get_p = |p: f64| -> f64 {
        let idx = (p / 100.0) * (n - 1) as f64;
        let lower = idx.floor() as usize;
        let upper = idx.ceil() as usize;
        if lower == upper {
            values[lower]
        } else {
            values[lower] + (values[upper] - values[lower]) * (idx - lower as f64)
        }
    };

    Some(Percentiles {
        count: n,
        p50: get_p(50.0),
        p95: get_p(95.0),
        p99: get_p(99.0),
        min: values[0],
        max: values[n - 1],
    })
}

pub fn execute(
    run: bool,
    log: Option<PathBuf>,
    schema: Option<String>,
    assert_p99_ms: Option<f64>,
    assert_p95_ms: Option<f64>,
) {
    if run {
        let script = Path::new("scripts/tests/benchmark_all_groups.sh");
        if script.exists() {
            println!("Chạy benchmark suite: {}...", script.display());
            let status = Command::new("bash").arg(script).status();
            match status {
                Ok(s) if !s.success() => {
                    println!("Quá trình benchmark kết thúc với mã: {:?}", s.code());
                }
                Err(e) => eprintln!("Lỗi khi chạy benchmark script: {}", e),
                _ => {}
            }
        }
    }

    let log_file = log.unwrap_or_else(|| PathBuf::from("/tmp/clak.log"));
    if !log_file.exists() {
        eprintln!(
            "Lỗi: Không tìm thấy file log '{}'. Hãy bật debug log hoặc dùng cờ --run",
            log_file.display()
        );
        exit(1);
    }

    let file = match File::open(&log_file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Lỗi khi đọc file log '{}': {}", log_file.display(), e);
            exit(1);
        }
    };

    let mut groups: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let reader = BufReader::new(file);

    for line_res in reader.lines() {
        if let Ok(line) = line_res {
            if let Some(idx) = line.find("[LATENCY]") {
                let rest = &line[idx + 9..];
                let tokens: Vec<&str> = rest.split_whitespace().collect();
                let mut group_val = None;
                let mut delta_val = None;
                let mut action_val = None;

                for token in tokens {
                    if let Some(stripped) = token.strip_prefix("group=") {
                        group_val = Some(stripped.to_string());
                    } else if let Some(stripped) = token.strip_prefix("delta_ms=") {
                        delta_val = stripped.parse::<f64>().ok();
                    } else if let Some(stripped) = token.strip_prefix("action=") {
                        action_val = Some(stripped.to_string());
                    }
                }

                if let (Some(g), Some(d), Some(a)) = (group_val, delta_val, action_val) {
                    if a.starts_with("REPLACE") {
                        groups.entry(g).or_default().push(d);
                    }
                }
            }
        }
    }

    if groups.is_empty() {
        eprintln!(
            "Không tìm thấy bản ghi latency nào trong {}",
            log_file.display()
        );
        exit(1);
    }

    let filtered_groups: BTreeMap<String, Vec<f64>> = if let Some(ref target) = schema {
        let lower_target = target.to_lowercase();
        let filtered: BTreeMap<_, _> = groups
            .into_iter()
            .filter(|(g, _)| g.to_lowercase().contains(&lower_target))
            .collect();
        if filtered.is_empty() {
            eprintln!("Không có bản ghi nào khớp với schema/group filter: '{}'", target);
            exit(1);
        }
        filtered
    } else {
        groups
    };

    println!(
        "\n{:<28} | {:<6} | {:<9} | {:<9} | {:<9} | {:<6} | {:<6}",
        "Group", "Count", "p50 (ms)", "p95 (ms)", "p99 (ms)", "Min", "Max"
    );
    println!("{}", "-".repeat(85));

    let mut assertion_failures = Vec::new();

    for (g, vals) in &filtered_groups {
        if let Some(stats) = calculate_percentiles(vals.clone()) {
            println!(
                "{:<28} | {:<6} | {:<9.2} | {:<9.2} | {:<9.2} | {:<6.2} | {:<6.2}",
                g, stats.count, stats.p50, stats.p95, stats.p99, stats.min, stats.max
            );

            if let Some(threshold) = assert_p99_ms {
                if stats.p99 > threshold {
                    assertion_failures.push(format!(
                        "Group '{}' p99 ({:.2}ms) vượt quá ngưỡng ({:.2}ms)",
                        g, stats.p99, threshold
                    ));
                }
            }

            if let Some(threshold) = assert_p95_ms {
                if stats.p95 > threshold {
                    assertion_failures.push(format!(
                        "Group '{}' p95 ({:.2}ms) vượt quá ngưỡng ({:.2}ms)",
                        g, stats.p95, threshold
                    ));
                }
            }
        }
    }

    println!("{}", "-".repeat(85));

    if !assertion_failures.is_empty() {
        println!("\n\x1b[31m❌ Kiểm tra benchmark THẤT BẠI:\x1b[0m");
        for fail in assertion_failures {
            println!("  - {}", fail);
        }
        exit(1);
    } else if assert_p99_ms.is_some() || assert_p95_ms.is_some() {
        println!("\n\x1b[32m✔ Tất cả kiểm tra ngưỡng latency đều ĐẠT!\x1b[0m");
        exit(0);
    }
}
