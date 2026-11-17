pub mod autocorrect;
pub mod autofix;
pub mod charset;
pub mod config;
pub mod core;
pub mod engine;
pub mod english_dict;
pub mod ffi;
pub mod ime;
pub mod learned_typos;
pub mod session;
pub mod spelling;
pub mod tables;
pub mod vseq;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use config::*;
pub use core::*;
pub use ffi::*;
pub use ime::*;
pub use session::*;
