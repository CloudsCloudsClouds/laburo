//! Deterministic MCP quality check-in.
//!
//! An agent can ask Laburo: what covers this feature, what ran, what has
//! evidence, what is broken, and what should happen next. No model call; the
//! same database state yields the same verdict.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckInFilter {
    pub q: Option<String>,
    pub case: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckIn {
    pub status: String,
    pub score: i64,
    pub matched_cases: i64,
    pub automated_cases: i64,
    pub manual_cases: i64,
    pub failing_runs: Vec<String>,
    pub open_bugs: Vec<String>,
    pub evidence_gaps: Vec<String>,
    pub next_actions: Vec<String>,
    pub matched_case_ids: Vec<String>,
}

struct CaseRow {
    id: String,
    title: String,
    driver_target: Option<String>,
    driver_steps: i64,
}

pub fn check_in(conn: &Connection, filter: &CheckInFilter) -> Result<CheckIn> {
    let cases = matched_cases(conn, filter)?;
    let mut failing_runs = Vec::new();
    let mut open_bug_refs = Vec::new();
    let mut evidence_gaps = Vec::new();
    let mut next_actions = Vec::new();
    let mut automated_cases = 0_i64;
    let mut manual_cases = 0_i64;

    for case in &cases {
        let automated = case.driver_target.is_some() || case.driver_steps > 0;
        if automated {
            automated_cases += 1;
        } else {
            manual_cases += 1;
            next_actions.push(format!(
                "Map {} ({}) to a discovered driver test; it is still manual-only.",
                case.id, case.title
            ));
        }

        match latest_run(conn, &case.id)? {
            None => {
                evidence_gaps.push(format!("{} has never run", case.id));
                next_actions.push(format!("Run {} and record evidence.", case.id));
            }
            Some((run_id, status)) => {
                if status == "failed" || status == "error" {
                    failing_runs.push(run_id.clone());
                    next_actions.push(format!("Fix {} for {}.", run_id, case.id));
                    if !run_has_evidence(conn, &run_id)? {
                        evidence_gaps.push(format!("{} failed without evidence", run_id));
                        next_actions.push(format!("Attach a log or screenshot to {}.", run_id));
                    }
                }
            }
        }

        if !case_has_expected_artifact(conn, &case.id)? {
            evidence_gaps.push(format!("{} has no expected artifact", case.id));
        }
    }

    for (bug_id, case_id, severity) in open_bugs(conn)? {
        if cases.is_empty()
            || case_id
                .as_deref()
                .is_some_and(|id| cases.iter().any(|case| case.id == id))
        {
            open_bug_refs.push(bug_id.clone());
            next_actions.push(format!(
                "Resolve {} (severity {}){}.",
                bug_id,
                severity,
                case_id
                    .as_deref()
                    .map(|id| format!(" on {id}"))
                    .unwrap_or_default()
            ));
        }
    }

    let critical_bugs = open_bug_count(conn, 1)? + open_bug_count(conn, 2)?;
    let status = if cases.is_empty() || !failing_runs.is_empty() || critical_bugs > 0 {
        "fail"
    } else if manual_cases > 0 || !evidence_gaps.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();

    let mut score = 100_i64;
    score -= 25 * failing_runs.len() as i64;
    score -= 25 * critical_bugs;
    score -= 10 * open_bug_refs.len() as i64;
    score -= 5 * manual_cases;
    score -= 5 * evidence_gaps.len() as i64;
    let score = score.clamp(0, 100);

    next_actions.sort();
    next_actions.dedup();
    evidence_gaps.sort();
    evidence_gaps.dedup();
    open_bug_refs.sort();
    open_bug_refs.dedup();
    failing_runs.sort();
    failing_runs.dedup();

    Ok(CheckIn {
        status,
        score,
        matched_cases: cases.len() as i64,
        automated_cases,
        manual_cases,
        failing_runs,
        open_bugs: open_bug_refs,
        evidence_gaps,
        next_actions,
        matched_case_ids: cases.into_iter().map(|case| case.id).collect(),
    })
}

fn matched_cases(conn: &Connection, filter: &CheckInFilter) -> Result<Vec<CaseRow>> {
    let sql = "
        SELECT c.id, c.title, c.driver_target,
               (SELECT count(*) FROM test_step s
                WHERE s.case_id = c.id AND s.kind = 'driver'
                  AND s.driver_target IS NOT NULL AND trim(s.driver_target) <> '')
        FROM test_case c
        WHERE c.status <> 'archived'
          AND (?1 IS NULL OR c.id = ?1)
          AND (?2 IS NULL OR c.id LIKE '%' || ?2 || '%'
               OR c.title LIKE '%' || ?2 || '%'
               OR c.module LIKE '%' || ?2 || '%'
               OR EXISTS (SELECT 1 FROM test_step s
                          WHERE s.case_id = c.id
                            AND (s.action LIKE '%' || ?2 || '%'
                                 OR s.expected LIKE '%' || ?2 || '%')))
          AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id = ct.tag_id
                WHERE ct.case_id = c.id AND t.name = ?3))
        ORDER BY c.id";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![filter.case, filter.q, filter.tag], |row| {
        Ok(CaseRow {
            id: row.get(0)?,
            title: row.get(1)?,
            driver_target: row.get(2)?,
            driver_steps: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn latest_run(conn: &Connection, case_id: &str) -> Result<Option<(String, String)>> {
    Ok(conn
        .query_row(
            "SELECT id, status FROM test_run WHERE case_id = ?1
             ORDER BY started_at DESC, id DESC LIMIT 1",
            [case_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

fn run_has_evidence(conn: &Connection, run_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM attachment a
         WHERE a.run_id = ?1
            OR a.step_run_id IN (SELECT id FROM step_run WHERE run_id = ?1)",
        [run_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn case_has_expected_artifact(conn: &Connection, case_id: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM attachment WHERE case_id = ?1 AND role = 'expected'",
        [case_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn open_bugs(conn: &Connection) -> Result<Vec<(String, Option<String>, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, case_id, severity FROM bug_report
         WHERE status IN ('open', 'reopened') ORDER BY severity, id",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn open_bug_count(conn: &Connection, severity: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM bug_report
         WHERE status IN ('open', 'reopened') AND severity = ?1",
        [severity],
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn empty_database_fails() {
        let db = Db::open_memory().unwrap();
        let check = check_in(&db.conn, &CheckInFilter::default()).unwrap();
        assert_eq!(check.status, "fail");
        assert_eq!(check.matched_cases, 0);
    }

    #[test]
    fn manual_case_warns() {
        let db = Db::open_memory().unwrap();
        db.conn
            .execute(
                "INSERT INTO test_case (id,title,status,created_at,updated_at)
                 VALUES ('TC-1','manual','active','now','now')",
                [],
            )
            .unwrap();
        let check = check_in(&db.conn, &CheckInFilter::default()).unwrap();
        assert_eq!(check.status, "warn");
        assert_eq!(check.manual_cases, 1);
        assert!(check.next_actions.iter().any(|a| a.contains("driver test")));
    }

    #[test]
    fn automated_pass_requires_no_failures() {
        let db = Db::open_memory().unwrap();
        db.conn
            .execute(
                "INSERT INTO test_case (id,title,status,driver_target,default_mode,created_at,updated_at)
                 VALUES ('TC-2','auto','active','UserTest#test_ok','single','now','now')",
                [],
            )
            .unwrap();
        let check = check_in(&db.conn, &CheckInFilter::default()).unwrap();
        // No run yet -> warn, not pass.
        assert_eq!(check.status, "warn");
        assert_eq!(check.automated_cases, 1);
    }
}
