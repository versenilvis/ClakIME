pub mod actions;
pub mod commands;

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "clak",
    author = "verse <versedev.store@proton.me>",
    version = "0.3.2",
    about = "Clak Vietnamese Input Method CLI Utility"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Inspect environment, permissions, fcitx5 daemon, and loaded binary")]
    Doctor {
        #[arg(long, help = "Output report as copyable Markdown")]
        markdown: bool,

        #[arg(long, help = "Output report as JSON")]
        json: bool,
    },

    #[command(about = "Benchmark latency analysis and regression assertion")]
    Bench {
        #[arg(long, help = "Run benchmark suite before analysis")]
        run: bool,

        #[arg(long, help = "Path to log file (default: /tmp/clak.log)")]
        log: Option<PathBuf>,

        #[arg(long, visible_alias = "group", help = "Filter by specific application group name")]
        schema: Option<String>,

        #[arg(long, help = "Assert that p99 latency does not exceed threshold in milliseconds")]
        assert_p99_ms: Option<f64>,

        #[arg(long, help = "Assert that p95 latency does not exceed threshold in milliseconds")]
        assert_p95_ms: Option<f64>,
    },

    #[command(about = "Configure autostart on system boot/login")]
    Autostart(AutostartArgs),

    #[command(about = "Launch Clak settings configuration GUI")]
    Gui {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Args, Debug)]
struct AutostartArgs {
    #[arg(help = "Action to perform: enable, disable, status")]
    action: Option<String>,

    #[arg(long, help = "Enable autostart on login")]
    enable: bool,

    #[arg(long, help = "Disable autostart on login")]
    disable: bool,

    #[arg(long, help = "Check autostart status")]
    status: bool,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor { markdown, json } => {
            commands::doctor::execute(markdown, json);
        }
        Commands::Bench {
            run,
            log,
            schema,
            assert_p99_ms,
            assert_p95_ms,
        } => {
            commands::bench::execute(run, log, schema, assert_p99_ms, assert_p95_ms);
        }
        Commands::Autostart(args) => {
            handle_autostart(args);
        }
        Commands::Gui { args } => {
            commands::gui::execute(args);
        }
    }
}

fn handle_autostart(args: AutostartArgs) {
    let mut action = args.action.as_deref();
    if args.enable {
        action = Some("enable");
    } else if args.disable {
        action = Some("disable");
    } else if args.status || action.is_none() {
        action = Some("status");
    }

    match action {
        Some("enable") => {
            match actions::autostart::enable_autostart() {
                Ok(_) => println!("\x1b[32m✔\x1b[0m Đã kích hoạt khởi động Clak cùng hệ thống"),
                Err(e) => eprintln!("Lỗi khi bật khởi động: {}", e),
            }
        }
        Some("disable") => {
            match actions::autostart::disable_autostart() {
                Ok(_) => println!("\x1b[32m✔\x1b[0m Đã tắt tự động chạy Clak Tiếng Việt cùng hệ thống"),
                Err(e) => eprintln!("Lỗi khi tắt khởi động: {}", e),
            }
        }
        _ => {
            let status = clak_diagnostics::check_autostart();
            let status_str = if status.is_enabled {
                "\x1b[32mĐang BẬT\x1b[0m"
            } else {
                "\x1b[33mĐang TẮT\x1b[0m"
            };
            println!("Trạng thái khởi động cùng hệ thống: {}", status_str);
        }
    }
}
