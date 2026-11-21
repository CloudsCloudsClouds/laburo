pub mod ai;
pub mod cli;
pub mod config;
pub mod db;
pub mod domain;
pub mod driver;
pub mod email;
pub mod import;
pub mod mcp;
pub mod mcp_quality;
pub mod model;
pub mod repo;
pub mod report;
pub mod tui;
pub mod web;

pub use model::{
    Attachment, BugReport, Dashboard, DiscoveredTest, RunMode, RunStatus, StepKind, StepRun,
    TestCase, TestOutcome, TestRun, TestStep,
};
