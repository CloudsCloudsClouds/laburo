//! Deterministic quality rules over SQLite.
//!
//! Every finding cites the records it came from and carries a stable id. The
//! same database state always yields the same findings in the same order:
//! severity first (critical, high, medium, low), then id.

use anyhow::Result;
use chrono::{DateTime, Local, NaiveDateTime};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A deterministic quality candidate. The AI may rank or explain it; it never
/// changes the cited records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub reason: String,
    pub refs: Vec<String>,
    pub suggested_action: String,
}

/// Number of days after which an open severity 1 or 2 bug is considered aged.
const AGING_DAYS: i64 = 7;

/// Rank a severity string. Higher means more urgent; unknown values rank lowest.
pub fn severity_rank(severity: &str) -> u8 {
    match severity.to_ascii_lowercase().as_str() {
        "critical" => 3,
        "high" => 2,
        "medium" => 1,
        _ => 0,
    }
}

/// Return the most urgent finding, deterministically.
///
/// Ordering is severity descending, then id ascending. No model call is made.
pub fn suggest_next(findings: &[Finding]) -> Option<&Finding> {
    findings.iter().min_by(|left, right| {
        severity_rank(&right.severity)
            .cmp(&severity_rank(&left.severity))
            .then_with(|| left.id.cmp(&right.id))
    })
}

/// Run every deterministic quality rule and return findings in a stable order.
pub fn quality_findings(conn: &Connection) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    findings.extend(case_without_steps(conn)?);
    findings.extend(step_without_expected(conn)?);
    findings.extend(failed_run_without_bug(conn)?);
    findings.extend(open_bug_without_steps(conn)?);
    findings.extend(case_without_tags(conn)?);
    findings.extend(case_never_run(conn)?);
    findings.extend(duplicate_case_titles(conn)?);
    findings.extend(aged_high_severity_bug(conn)?);
    findings.sort_by(|left, right| {
        severity_rank(&right.severity)
            .cmp(&severity_rank(&left.severity))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(findings)
}

fn case_without_steps(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title
         FROM test_case c
         WHERE c.status <> 'archived'
           AND NOT EXISTS (SELECT 1 FROM test_step s WHERE s.case_id = c.id)
         ORDER BY c.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (id, title) = row?;
        findings.push(Finding {
            id: format!("case-no-steps:{id}"),
            kind: "case-no-steps".to_string(),
            severity: "high".to_string(),
            reason: format!("case {id} (\"{title}\") has no steps"),
            refs: vec![id.clone()],
            suggested_action: format!(
                "Add the ordered actions and expected results for {id}, or archive the case."
            ),
        });
    }
    Ok(findings)
}

fn step_without_expected(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT s.case_id, s.id, s.position
         FROM test_step s
         JOIN test_case c ON c.id = s.case_id
         WHERE trim(s.expected) = ''
           AND c.status <> 'archived'
         ORDER BY s.case_id, s.position",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (case_id, step_id, position) = row?;
        findings.push(Finding {
            id: format!("step-no-expected:{case_id}:{position}"),
            kind: "step-no-expected".to_string(),
            severity: "medium".to_string(),
            reason: format!("step {position} of case {case_id} has no expected result"),
            refs: vec![case_id.clone(), format!("step:{step_id}")],
            suggested_action: format!(
                "State the observable expected result for step {position} of {case_id}."
            ),
        });
    }
    Ok(findings)
}

fn failed_run_without_bug(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.case_id, r.status
         FROM test_run r
         WHERE r.status IN ('failed', 'error')
           AND NOT EXISTS (
                SELECT 1 FROM bug_report b
                WHERE b.run_id = r.id
                   OR b.step_run_id IN (SELECT sr.id FROM step_run sr WHERE sr.run_id = r.id))
         ORDER BY r.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (run_id, case_id, status) = row?;
        findings.push(Finding {
            id: format!("failed-run-no-bug:{run_id}"),
            kind: "failed-run-no-bug".to_string(),
            severity: "high".to_string(),
            reason: format!("{status} run {run_id} of case {case_id} has no linked bug"),
            refs: vec![run_id.clone(), case_id.clone()],
            suggested_action: format!(
                "File a bug from the observed failures in {run_id} and link it to the run."
            ),
        });
    }
    Ok(findings)
}

fn open_bug_without_steps(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT id, case_id, run_id, status
         FROM bug_report
         WHERE status IN ('open', 'reopened')
           AND trim(steps) = ''
         ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (id, case_id, run_id, status) = row?;
        let mut refs = vec![id.clone()];
        if let Some(case_id) = case_id {
            refs.push(case_id);
        }
        if let Some(run_id) = run_id {
            refs.push(run_id);
        }
        findings.push(Finding {
            id: format!("bug-no-steps:{id}"),
            kind: "bug-no-steps".to_string(),
            severity: "high".to_string(),
            reason: format!("{status} bug {id} has no reproduction steps"),
            refs,
            suggested_action: format!(
                "Add reproduction steps plus observed and expected behaviour to {id}."
            ),
        });
    }
    Ok(findings)
}

fn case_without_tags(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title
         FROM test_case c
         WHERE c.status <> 'archived'
           AND NOT EXISTS (SELECT 1 FROM case_tag ct WHERE ct.case_id = c.id)
         ORDER BY c.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (id, title) = row?;
        findings.push(Finding {
            id: format!("case-no-tags:{id}"),
            kind: "case-no-tags".to_string(),
            severity: "low".to_string(),
            reason: format!("case {id} (\"{title}\") has no tags"),
            refs: vec![id.clone()],
            suggested_action: format!(
                "Tag {id} with at least one level and one type (for example unit/functional)."
            ),
        });
    }
    Ok(findings)
}

fn case_never_run(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.title
         FROM test_case c
         WHERE c.status <> 'archived'
           AND NOT EXISTS (SELECT 1 FROM test_run r WHERE r.case_id = c.id)
         ORDER BY c.id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut findings = Vec::new();
    for row in rows {
        let (id, title) = row?;
        findings.push(Finding {
            id: format!("case-never-run:{id}"),
            kind: "case-never-run".to_string(),
            severity: "medium".to_string(),
            reason: format!("case {id} (\"{title}\") has never been run"),
            refs: vec![id.clone()],
            suggested_action: format!("Execute {id} in a run and record the result of every step."),
        });
    }
    Ok(findings)
}

fn duplicate_case_titles(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT id, title FROM test_case
         WHERE status <> 'archived'
         ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for row in rows {
        let (id, title) = row?;
        groups
            .entry(normalize_title(&title))
            .or_default()
            .push((id, title));
    }
    let mut findings = Vec::new();
    for group in groups.values() {
        if group.len() < 2 {
            continue;
        }
        let (first_id, first_title) = &group[0];
        let ids: Vec<String> = group.iter().map(|(id, _)| id.clone()).collect();
        findings.push(Finding {
            id: format!("duplicate-case-title:{first_id}"),
            kind: "duplicate-case-title".to_string(),
            severity: "low".to_string(),
            reason: format!(
                "cases {} share the title \"{}\"",
                ids.join(", "),
                first_title.trim()
            ),
            refs: ids.clone(),
            suggested_action: format!(
                "Rename or merge cases {} so each title identifies one case.",
                ids.join(", ")
            ),
        });
    }
    Ok(findings)
}

fn aged_high_severity_bug(conn: &Connection) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT id, severity, created_at, case_id, run_id
         FROM bug_report
         WHERE status IN ('open', 'reopened')
           AND severity IN (1, 2)
         ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let now = Local::now().naive_local();
    let threshold = chrono::Duration::days(AGING_DAYS);
    let mut findings = Vec::new();
    for row in rows {
        let (id, severity, created_at, case_id, run_id) = row?;
        // Timestamps that do not parse are skipped rather than guessed at.
        let Some(created) = parse_timestamp(&created_at) else {
            continue;
        };
        if now.signed_duration_since(created) < threshold {
            continue;
        }
        let mut refs = vec![id.clone()];
        if let Some(case_id) = case_id {
            refs.push(case_id);
        }
        if let Some(run_id) = run_id {
            refs.push(run_id);
        }
        findings.push(Finding {
            id: format!("bug-aging:{id}"),
            kind: "bug-aging".to_string(),
            severity: "critical".to_string(),
            reason: format!(
                "open severity {severity} bug {id} has been open for at least {AGING_DAYS} days"
            ),
            refs,
            suggested_action: format!(
                "Triage {id}: fix it, record a justified severity change, or close it."
            ),
        });
    }
    Ok(findings)
}

fn normalize_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn parse_timestamp(raw: &str) -> Option<NaiveDateTime> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
    ] {
        if let Ok(timestamp) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(timestamp);
        }
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Local).naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::model::{RunMode, RunStatus, StepKind};
    use crate::repo;
    use rusqlite::params;

    fn find<'a>(findings: &'a [Finding], id: &str) -> &'a Finding {
        findings
            .iter()
            .find(|finding| finding.id == id)
            .unwrap_or_else(|| panic!("missing finding {id}"))
    }

    fn sample_finding(id: &str, severity: &str) -> Finding {
        Finding {
            id: id.to_string(),
            kind: "sample".to_string(),
            severity: severity.to_string(),
            reason: String::new(),
            refs: Vec::new(),
            suggested_action: String::new(),
        }
    }

    #[test]
    fn empty_database_has_no_findings() {
        let db = Db::open_memory().unwrap();
        assert!(quality_findings(&db.conn).unwrap().is_empty());
    }

    #[test]
    fn case_without_steps_tags_or_runs_is_flagged() {
        let db = Db::open_memory().unwrap();
        repo::create_case(
            &db.conn,
            "TC-001",
            "Login works",
            "",
            "auth",
            "medium",
            "local",
            "",
            &[],
        )
        .unwrap();

        let findings = quality_findings(&db.conn).unwrap();
        let ids: Vec<&str> = findings.iter().map(|finding| finding.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "case-no-steps:TC-001",
                "case-never-run:TC-001",
                "case-no-tags:TC-001",
            ]
        );

        let no_steps = find(&findings, "case-no-steps:TC-001");
        assert_eq!(no_steps.severity, "high");
        assert_eq!(no_steps.refs, vec!["TC-001".to_string()]);
        assert!(!no_steps.suggested_action.is_empty());
        assert_eq!(find(&findings, "case-never-run:TC-001").severity, "medium");
        assert_eq!(find(&findings, "case-no-tags:TC-001").severity, "low");
    }

    #[test]
    fn step_without_expected_result_is_flagged() {
        let db = Db::open_memory().unwrap();
        repo::create_case(
            &db.conn,
            "TC-010",
            "Checkout",
            "",
            "cart",
            "high",
            "local",
            "",
            &["smoke".to_string()],
        )
        .unwrap();
        let step = repo::add_step(
            &db.conn,
            "TC-010",
            1,
            "Open the cart",
            "",
            "",
            StepKind::Manual,
            None,
        )
        .unwrap();

        let findings = quality_findings(&db.conn).unwrap();
        let finding = find(&findings, "step-no-expected:TC-010:1");
        assert_eq!(finding.severity, "medium");
        assert_eq!(
            finding.refs,
            vec!["TC-010".to_string(), format!("step:{}", step.id)]
        );
    }

    #[test]
    fn failed_run_without_bug_is_flagged_until_a_bug_is_linked() {
        let db = Db::open_memory().unwrap();
        repo::create_case(
            &db.conn,
            "TC-020",
            "Submit form",
            "",
            "orders",
            "high",
            "local",
            "",
            &["smoke".to_string()],
        )
        .unwrap();
        repo::add_step(
            &db.conn,
            "TC-020",
            1,
            "Press submit",
            "",
            "Confirmation appears",
            StepKind::Manual,
            None,
        )
        .unwrap();
        let run = repo::create_run(&db.conn, "TC-020", RunMode::Manual, "local").unwrap();
        repo::record_step(
            &db.conn,
            &run.id,
            1,
            RunStatus::Failed,
            "No confirmation",
            "",
            None,
            None,
            "",
        )
        .unwrap();
        let finished = repo::finish_run(&db.conn, &run.id).unwrap();
        assert_eq!(finished.status, RunStatus::Failed);

        let run_finding = format!("failed-run-no-bug:{}", run.id);
        let findings = quality_findings(&db.conn).unwrap();
        let finding = find(&findings, &run_finding);
        assert_eq!(finding.severity, "high");
        assert_eq!(finding.refs, vec![run.id.clone(), "TC-020".to_string()]);
        assert!(
            findings
                .iter()
                .all(|candidate| candidate.id != "case-never-run:TC-020")
        );

        repo::create_bug(
            &db.conn,
            Some("BUG-020"),
            "Submit does nothing",
            "",
            "1. Press submit",
            "No confirmation",
            "Confirmation appears",
            2,
            3,
            "local",
            Some("TC-020"),
            Some(&run.id),
            None,
            "",
        )
        .unwrap();

        let linked = quality_findings(&db.conn).unwrap();
        assert!(linked.iter().all(|candidate| candidate.id != run_finding));
    }

    #[test]
    fn open_bug_without_steps_is_flagged() {
        let db = Db::open_memory().unwrap();
        repo::create_bug(
            &db.conn,
            Some("BUG-030"),
            "Crash on save",
            "",
            "",
            "The application crashes",
            "The record is saved",
            2,
            3,
            "local",
            None,
            None,
            None,
            "",
        )
        .unwrap();

        let findings = quality_findings(&db.conn).unwrap();
        let finding = find(&findings, "bug-no-steps:BUG-030");
        assert_eq!(finding.severity, "high");
        assert_eq!(finding.refs, vec!["BUG-030".to_string()]);
    }

    #[test]
    fn duplicate_case_titles_are_grouped() {
        let db = Db::open_memory().unwrap();
        for id in ["TC-050", "TC-051"] {
            repo::create_case(
                &db.conn,
                id,
                "  Login Works ",
                "",
                "auth",
                "medium",
                "local",
                "",
                &["smoke".to_string()],
            )
            .unwrap();
        }

        let findings = quality_findings(&db.conn).unwrap();
        let finding = find(&findings, "duplicate-case-title:TC-050");
        assert_eq!(finding.severity, "low");
        assert_eq!(
            finding.refs,
            vec!["TC-050".to_string(), "TC-051".to_string()]
        );
        assert!(finding.reason.contains("Login Works"));
    }

    #[test]
    fn aged_high_severity_bug_requires_a_parsable_timestamp() {
        let db = Db::open_memory().unwrap();
        repo::create_bug(
            &db.conn,
            Some("BUG-040"),
            "Data loss",
            "",
            "1. Save",
            "The record is lost",
            "The record is saved",
            1,
            1,
            "production",
            None,
            None,
            None,
            "",
        )
        .unwrap();
        db.conn
            .execute(
                "UPDATE bug_report SET created_at=?1 WHERE id=?2",
                params!["2000-01-01 00:00:00", "BUG-040"],
            )
            .unwrap();

        let findings = quality_findings(&db.conn).unwrap();
        let finding = find(&findings, "bug-aging:BUG-040");
        assert_eq!(finding.severity, "critical");
        assert!(finding.reason.contains("severity 1"));

        repo::create_bug(
            &db.conn,
            Some("BUG-041"),
            "Bad timestamp",
            "",
            "1. Save",
            "The record is lost",
            "The record is saved",
            1,
            1,
            "production",
            None,
            None,
            None,
            "",
        )
        .unwrap();
        db.conn
            .execute(
                "UPDATE bug_report SET created_at='not a timestamp' WHERE id='BUG-041'",
                [],
            )
            .unwrap();

        let findings = quality_findings(&db.conn).unwrap();
        assert!(
            findings
                .iter()
                .all(|candidate| candidate.id != "bug-aging:BUG-041")
        );
    }

    #[test]
    fn suggest_next_orders_by_severity_then_id() {
        let findings = vec![
            sample_finding("b-low", "low"),
            sample_finding("z-high", "high"),
            sample_finding("a-high", "high"),
            sample_finding("c-critical", "critical"),
        ];

        let next = suggest_next(&findings).unwrap();
        assert_eq!(next.id, "c-critical");
        assert!(suggest_next(&[]).is_none());
        assert!(severity_rank("critical") > severity_rank("high"));
        assert_eq!(severity_rank("unknown"), 0);
    }
}
