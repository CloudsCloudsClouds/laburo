//! Test drivers.
//!
//! A driver turns a selected test into a runnable target and a parsed result.
//! Manual runs are owned by the TUI; Minitest is the first automated driver.
//! Cargo nextest is exposed as a boundary and returns an actionable error until
//! it is implemented.

pub mod manual;
pub mod minitest;
pub mod nextest;

pub use manual::ManualDriver;
pub use minitest::{MinitestDriver, parse_minitest_output, strip_ansi};
pub use nextest::NextestDriver;

use crate::model::{DiscoveredTest, TestOutcome};
use anyhow::{Result, bail};
use std::path::Path;
use std::time::Duration;

/// Default hard limit for one driver invocation.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Options shared by automated driver runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    /// Hard limit for one driver invocation. A timeout is an `error` result,
    /// never a hang.
    pub timeout: Duration,
    /// Force the system-test task (`bin/rails test:system`) even when the
    /// target path does not mention `test/system`.
    pub system: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            system: false,
        }
    }
}

impl RunOptions {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn system(mut self) -> Self {
        self.system = true;
        self
    }
}

/// What a driver can do. The TUI/CLI use this to enable or disable actions
/// instead of guessing from the driver id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Can list tests from the project.
    pub discovery: bool,
    /// Can execute automated tests.
    pub execution: bool,
    /// Can run Rails system tests.
    pub system_tests: bool,
    /// Enforces a timeout and kills the child process.
    pub timeouts: bool,
}

impl Capabilities {
    pub const NONE: Self = Self {
        discovery: false,
        execution: false,
        system_tests: false,
        timeouts: false,
    };

    pub const MANUAL: Self = Self::NONE;

    pub const MINITEST: Self = Self {
        discovery: true,
        execution: true,
        system_tests: true,
        timeouts: true,
    };

    /// The boundary exists, but this wave does not implement it.
    pub const NEXTEST: Self = Self::NONE;
}

/// A test driver.
///
/// Drivers never pass user text through a shell: implementations build
/// [`std::process::Command`] with an argv list.
pub trait Driver {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> Capabilities;

    /// List the tests this driver knows about.
    ///
    /// Drivers without discovery (manual, nextest boundary) return an
    /// actionable error rather than silently returning an empty list.
    fn discover(&self, _project: &Path) -> Result<Vec<DiscoveredTest>> {
        bail!(
            "driver '{}' does not support discovery; use a driver with discovery or enter a target manually",
            self.id()
        )
    }

    /// Execute the given targets.
    ///
    /// Implementations must capture stdout/stderr and enforce
    /// [`RunOptions::timeout`].
    fn run(
        &self,
        _project: &Path,
        _targets: &[String],
        _options: &RunOptions,
    ) -> Result<Vec<TestOutcome>> {
        bail!(
            "driver '{}' does not support automated execution; record the result manually",
            self.id()
        )
    }
}

/// Convenience wrapper kept for callers of the Wave 0 skeleton.
pub fn discover_minitest(project: &Path) -> Result<Vec<DiscoveredTest>> {
    MinitestDriver.discover(project)
}

/// Convenience wrapper kept for callers of the Wave 0 skeleton.
pub fn run_minitest(project: &Path, targets: &[String]) -> Result<Vec<TestOutcome>> {
    MinitestDriver.run(project, targets, &RunOptions::default())
}
