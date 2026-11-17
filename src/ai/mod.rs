//! Deterministic quality rules and the optional AI provider boundary.
//!
//! Tier 1 works without a model: [`quality_findings`] reads SQLite and returns
//! explainable candidates, and [`suggest_next`] ranks them deterministically.
//! AI is disabled by default ([`AiConfig::default`]); when disabled, providers
//! make no network calls.
//!
//! The "failed run but the report was generated as green" rule is intentionally
//! not implemented: reports are a transient projection, not a SQLite record, so
//! the rule would need filesystem state and would not be deterministic.

mod provider;
mod rules;

pub use provider::{
    AiConfig, AiProvider, NOOP_FALLBACK, NoopProvider, OpenAiProvider, provider_from_config,
};
pub use rules::{Finding, quality_findings, severity_rank, suggest_next};
