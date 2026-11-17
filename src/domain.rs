//! Framework-neutral test evidence.
//!
//! Nothing here knows about Minitest, Cargo, YAML, or Typst. Adapters translate
//! native runner output into these types; renderers consume them.

/// The test framework that produced a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    Minitest,
    CargoTest,
}

impl Framework {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Minitest => "minitest",
            Self::CargoTest => "cargo test",
        }
    }
}

/// Outcome of a single test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Passed,
    Failed,
    Skipped,
    Errored,
}

/// One test as reported by a runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    /// Stable identity: `Class#method` for Minitest, `path::test_name` for Cargo.
    pub id: String,
    pub status: Status,
    /// Duration in milliseconds when the runner reports it.
    pub duration_ms: Option<u64>,
    /// Source location, when the runner reports it.
    pub location: Option<String>,
}

impl TestResult {
    #[must_use]
    pub fn new(id: impl Into<String>, status: Status) -> Self {
        Self {
            id: id.into(),
            status,
            duration_ms: None,
            location: None,
        }
    }

    #[must_use]
    pub const fn passed(&self) -> bool {
        matches!(self.status, Status::Passed)
    }
}

/// A collection of results produced by one invocation of a test framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRun {
    pub framework: Framework,
    pub tests: Vec<TestResult>,
}

impl TestRun {
    #[must_use]
    pub fn new(framework: Framework, tests: Vec<TestResult>) -> Self {
        Self { framework, tests }
    }

    #[must_use]
    pub fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for test in &self.tests {
            match test.status {
                Status::Passed => counts.passed += 1,
                Status::Failed => counts.failed += 1,
                Status::Skipped => counts.skipped += 1,
                Status::Errored => counts.errored += 1,
            }
        }
        counts
    }
}

/// Aggregate counts for a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errored: usize,
}

impl Counts {
    #[must_use]
    pub const fn total(self) -> usize {
        self.passed + self.failed + self.skipped + self.errored
    }

    /// A run is green when nothing failed and nothing errored; skips are allowed.
    #[must_use]
    pub const fn green(self) -> bool {
        self.failed == 0 && self.errored == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_every_status() {
        let run = TestRun::new(
            Framework::Minitest,
            vec![
                TestResult::new("BookingTest#persists", Status::Passed),
                TestResult::new("BookingTest#rejects", Status::Failed),
                TestResult::new("BookingTest#later", Status::Skipped),
                TestResult::new("BookingTest#boom", Status::Errored),
            ],
        );

        let counts = run.counts();
        assert_eq!(
            counts,
            Counts {
                passed: 1,
                failed: 1,
                skipped: 1,
                errored: 1,
            }
        );
        assert_eq!(counts.total(), 4);
        assert!(!counts.green());
    }

    #[test]
    fn green_when_nothing_failed() {
        let run = TestRun::new(
            Framework::CargoTest,
            vec![
                TestResult::new("domain::tests::ok", Status::Passed),
                TestResult::new("domain::tests::ignored", Status::Skipped),
            ],
        );

        assert!(run.counts().green());
        assert!(run.tests[0].passed());
    }
}
