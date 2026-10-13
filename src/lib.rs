//! Die package identity and project graph, independent of compiler execution.

pub mod manifest;
pub mod project;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
