use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn execute(args: Vec<String>) {
    let current_exe = env::current_exe().unwrap_or_default();
    let repo_root = current_exe
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    let search_paths = [
        repo_root.join("build/cargo-target/release/clak-gui"),
        repo_root.join("ui/target/release/clak-gui"),
        repo_root.join("build/ui-target/release/clak-gui"),
        repo_root.join("ui/target/debug/clak-gui"),
        PathBuf::from("build/cargo-target/release/clak-gui"),
        PathBuf::from("ui/target/release/clak-gui"),
        PathBuf::from("clak-gui"),
    ];

    for candidate in &search_paths {
        if candidate.exists() {
            exec_binary(candidate, &args);
            return;
        }
    }

    // fallback to cargo run
    let manifest = repo_root.join("ui/Cargo.toml");
    let manifest_path = if manifest.exists() {
        manifest
    } else {
        PathBuf::from("ui/Cargo.toml")
    };

    println!("Không tìm thấy binary clak-gui biên dịch sẵn, khởi chạy qua cargo...");
    let _ = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--release")
        .args(args)
        .status();
}

fn exec_binary(bin: &Path, args: &[String]) {
    let mut c_args = Vec::new();
    let bin_str = bin.to_string_lossy();
    if let Ok(c_bin) = CString::new(bin_str.as_bytes()) {
        c_args.push(c_bin);
        for a in args {
            if let Ok(ca) = CString::new(a.as_bytes()) {
                c_args.push(ca);
            }
        }
        let c_ptrs: Vec<*const libc::c_char> = c_args
            .iter()
            .map(|c| c.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        unsafe {
            libc::execvp(c_args[0].as_ptr(), c_ptrs.as_ptr());
        }
    }

    // fallback if execvp fails
    let _ = Command::new(bin).args(args).status();
}
