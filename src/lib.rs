//! Die package identity and project graph, independent of compiler execution.

mod cache;
mod git;
pub mod manifest;
pub mod project;
pub mod runner;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
