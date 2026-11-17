use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Manual,
    Single,
    Composed,
}

impl RunMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Single => "single",
            Self::Composed => "composed",
        }
    }
}

impl FromStr for RunMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "single" => Ok(Self::Single),
            "composed" => Ok(Self::Composed),
            other => Err(format!("unknown run mode: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    NotRun,
    Running,
    Passed,
    Failed,
    Blocked,
    Error,
}

impl RunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRun => "not_run",
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Error => "error",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Passed | Self::Failed | Self::Blocked | Self::Error
        )
    }

    pub const fn is_bad(self) -> bool {
        matches!(self, Self::Failed | Self::Error)
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RunStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "not_run" | "not-run" => Ok(Self::NotRun),
            "running" => Ok(Self::Running),
            "passed" | "pass" | "green" => Ok(Self::Passed),
            "failed" | "fail" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            "error" => Ok(Self::Error),
            other => Err(format!("unknown status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    Manual,
    Driver,
}

impl StepKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Driver => "driver",
        }
    }
}

impl FromStr for StepKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(Self::Manual),
            "driver" => Ok(Self::Driver),
            other => Err(format!("unknown step kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestStep {
    pub id: i64,
    pub case_id: String,
    pub position: i64,
    pub action: String,
    pub data: String,
    pub expected: String,
    pub kind: StepKind,
    pub driver_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub title: String,
    pub description: String,
    pub module: String,
    pub priority: String,
    pub status: String,
    pub environment: String,
    pub gherkin: String,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub steps: Vec<TestStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: i64,
    pub kind: String,
    pub path: String,
    pub original_name: String,
    pub mime: String,
    pub sha256: String,
    pub size: i64,
    pub caption: String,
    pub created_at: String,
    pub step_run_id: Option<i64>,
    pub run_id: Option<String>,
    pub bug_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepRun {
    pub id: i64,
    pub run_id: String,
    pub step_id: Option<i64>,
    pub position: i64,
    pub status: RunStatus,
    pub observed: String,
    pub notes: String,
    pub duration_ms: Option<i64>,
    pub driver_test: Option<String>,
    pub raw_output: String,
    pub evidence: Vec<Attachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRun {
    pub id: String,
    pub case_id: String,
    pub mode: RunMode,
    pub status: RunStatus,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub environment: String,
    pub commit_ref: String,
    pub command: String,
    pub notes: String,
    pub steps: Vec<StepRun>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugReport {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: String,
    pub actual: String,
    pub expected: String,
    pub priority: i64,
    pub severity: i64,
    pub environment: String,
    pub status: String,
    pub case_id: Option<String>,
    pub run_id: Option<String>,
    pub step_run_id: Option<i64>,
    pub comments: String,
    pub created_at: String,
    pub updated_at: String,
    pub evidence: Vec<Attachment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredTest {
    pub id: String,
    pub driver_id: String,
    pub name: String,
    pub file: String,
    pub line: Option<i64>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestOutcome {
    pub target: String,
    pub status: RunStatus,
    pub duration_ms: Option<i64>,
    pub message: String,
    pub backtrace: String,
    pub raw_output: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dashboard {
    pub cases: i64,
    pub runs: i64,
    pub bugs_open: i64,
    pub bugs_total: i64,
    pub failed_runs: i64,
    pub passed_runs: i64,
    pub evidence: i64,
}

impl Dashboard {
    pub fn pass_rate(&self) -> f64 {
        let total = self.passed_runs + self.failed_runs;
        if total == 0 {
            100.0
        } else {
            self.passed_runs as f64 / total as f64 * 100.0
        }
    }
}

pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
