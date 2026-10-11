//! The Melomae core pipeline.
//!
//! This crate holds decode, inference, encode, triage, tag, provenance,
//! and job code. It must not depend on any `tauri`
//! crate. The `no_tauri_dependency` test enforces that rule.

pub mod decode;
pub mod encode;
pub mod inference;
pub mod store;
pub mod triage;

/// The crate version, read from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
