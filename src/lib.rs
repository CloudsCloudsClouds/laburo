//! Laburo's library: the core domain and the adapters that feed it.
//!
//! The binary in `main.rs` is deliberately thin. Everything a TUI, a script, or
//! a test needs to call lives here.

pub mod cli;
pub mod domain;

pub use domain::{Counts, Framework, Status, TestResult, TestRun};
