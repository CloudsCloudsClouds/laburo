//! Manual driver.
//!
//! Manual runs have no test discovery and no child process. The TUI owns the
//! step loop: the user marks pass/fail/blocked and records notes and evidence.
//! Calling [`Driver::discover`] or [`Driver::run`] on this driver returns an
//! actionable error through the trait defaults.

use super::{Capabilities, Driver};

#[derive(Debug, Default, Clone, Copy)]
pub struct ManualDriver;

impl Driver for ManualDriver {
    fn id(&self) -> &str {
        "manual"
    }

    fn name(&self) -> &str {
        "Manual"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::MANUAL
    }
}
