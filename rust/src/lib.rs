pub mod cli;
pub mod error;
pub mod handoff;
pub mod hook_config;
pub mod protocol;
pub mod reasoning;
pub mod server;
pub mod sse;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_UPSTREAM: &str = "https://api.kimi.com/coding/v1/chat/completions";
