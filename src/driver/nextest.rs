//! Cargo nextest driver boundary.
//!
//! The interface is deliberately present so the TUI/CLI can register nextest
//! alongside the other drivers. Execution is not implemented in this wave; both
//! discovery and run return an actionable error instead of pretending to work.

use super::{Capabilities, Driver};
use crate::model::{DiscoveredTest, TestOutcome};
use anyhow::{Result, bail};
use std::path::Path;

#[derive(Debug, Default, Clone, Copy)]
pub struct NextestDriver;

const NOT_IMPLEMENTED: &str = "cargo-nextest driver is not implemented in this wave; \
run `cargo nextest list --message-format json` or `cargo nextest run` directly for now";

impl Driver for NextestDriver {
    fn id(&self) -> &str {
        "nextest"
    }

    fn name(&self) -> &str {
        "cargo nextest"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NEXTEST
    }

    fn discover(&self, _project: &Path) -> Result<Vec<DiscoveredTest>> {
        bail!("{NOT_IMPLEMENTED}")
    }

    fn run(
        &self,
        _project: &Path,
        _targets: &[String],
        _options: &super::RunOptions,
    ) -> Result<Vec<TestOutcome>> {
        bail!("{NOT_IMPLEMENTED}")
    }
}
