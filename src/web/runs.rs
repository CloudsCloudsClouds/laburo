//! Run list, run sheet, manual step recording and run evidence.

use super::data::{self, EvidenceTarget};
use super::{
    AppState, WebError, clean, clean_or, collect_multipart, db, flash_param, json_value, redirect,
    render, with_upload,
};
use crate::model::{RunStatus, TestRun};
use crate::repo;
use axum::extract::{Form, Multipart, Path, Query, State};
use axum::response::{Html, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::str::FromStr;

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub status: Option<String>,
    pub case_id: Option<String>,
    pub flash: Option<String>,
}

fn matches_filter(run: &TestRun, params: &ListParams) -> bool {
    if let Some(status) = clean(params.status.as_deref())
        && run.status.as_str() != status
    {
        return false;
    }
    if let Some(case_id) = clean(params.case_id.as_deref())
        && run.case_id != case_id
    {
        return false;
    }
    if let Some(query) = clean(params.q.as_deref()) {
        let needle = query.to_lowercase();
        if !run.id.to_lowercase().contains(&needle)
            && !run.case_id.to_lowercase().contains(&needle)
            && !run.environment.to_lowercase().contains(&needle)
        {
            return false;
        }
    }
    true
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let runs = repo::list_runs(&conn, None, 300)?;
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    for run in &runs {
        *counts.entry(run.status.as_str().to_string()).or_default() += 1;
    }
    let mut rows = Vec::new();
    for run in runs.iter().filter(|run| matches_filter(run, &params)) {
        let mut value = json_value(run)?;
        if let Value::Object(map) = &mut value {
            let passed = run
                .steps
                .iter()
                .filter(|step| step.status == RunStatus::Passed)
                .count();
            let failed = run
                .steps
                .iter()
                .filter(|step| step.status.is_bad() || step.status == RunStatus::Blocked)
                .count();
            let evidence: usize = run.steps.iter().map(|step| step.evidence.len()).sum();
            map.insert("passed_steps".into(), json!(passed));
            map.insert("failed_steps".into(), json!(failed));
            map.insert("step_count".into(), json!(run.steps.len()));
            map.insert("step_evidence".into(), json!(evidence));
        }
        rows.push(value);
    }
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "runs.html",
        "runs",
        flash.as_deref(),
        json!({
            "runs": rows,
            "total": runs.len(),
            "shown": rows.len(),
            "counts": counts,
            "filters": {
                "q": params.q, "status": params.status, "case_id": params.case_id
            },
            "filters_active": clean(params.q.as_deref()).is_some()
                || clean(params.status.as_deref()).is_some()
                || clean(params.case_id.as_deref()).is_some(),
        }),
    )
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let run = repo::get_run(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("run {id} not found")))?;
    let case = repo::get_case(&conn, &run.case_id)?;
    let evidence = data::evidence_for(&conn, &EvidenceTarget::Run(id.clone()))?;
    let links = data::list_links(&conn, "run", &id)?;
    let all_bugs = repo::list_bugs(&conn, None)?;
    let bugs: Vec<_> = all_bugs
        .into_iter()
        .filter(|bug| bug.run_id.as_deref() == Some(id.as_str()))
        .collect();

    let mut steps = Vec::new();
    for step in &run.steps {
        let mut value = json_value(step)?;
        let case_step = case.as_ref().and_then(|case| {
            case.steps
                .iter()
                .find(|item| item.position == step.position)
        });
        let evidence: Vec<_> = step
            .evidence
            .iter()
            .map(|item| data::evidence_for_attachment(&conn, item))
            .collect();
        let step_bugs: Vec<_> = bugs
            .iter()
            .filter(|bug| bug.step_run_id == Some(step.id))
            .map(|bug| bug.id.clone())
            .collect();
        if let Value::Object(map) = &mut value {
            map.insert(
                "action".into(),
                json!(
                    case_step
                        .map(|item| item.action.clone())
                        .unwrap_or_default()
                ),
            );
            map.insert(
                "data".into(),
                json!(case_step.map(|item| item.data.clone()).unwrap_or_default()),
            );
            map.insert(
                "expected".into(),
                json!(
                    case_step
                        .map(|item| item.expected.clone())
                        .unwrap_or_default()
                ),
            );
            map.insert(
                "step_kind".into(),
                json!(
                    case_step
                        .map(|item| item.kind.as_str().to_string())
                        .unwrap_or_else(|| "manual".to_string())
                ),
            );
            map.insert("evidence_items".into(), json_value(&evidence)?);
            map.insert("bugs".into(), json!(step_bugs));
        }
        steps.push(value);
    }

    let passed = run
        .steps
        .iter()
        .filter(|step| step.status == RunStatus::Passed)
        .count();
    let failed = run
        .steps
        .iter()
        .filter(|step| step.status.is_bad() || step.status == RunStatus::Blocked)
        .count();
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "run_detail.html",
        "runs",
        flash.as_deref(),
        json!({
            "run": json_value(&run)?,
            "steps": steps,
            "case": case.map(|case| json_value(&case)).transpose()?,
            "evidence": evidence,
            "links": links,
            "bugs": bugs,
            "passed": passed,
            "failed": failed,
            "finished": run.status.is_terminal(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct StepRecordForm {
    pub status: Option<String>,
    pub observed: Option<String>,
    pub notes: Option<String>,
    pub duration_ms: Option<String>,
    pub driver_test: Option<String>,
    pub raw_output: Option<String>,
}

pub async fn record_step(
    State(state): State<AppState>,
    Path((id, position)): Path<(String, i64)>,
    Form(form): Form<StepRecordForm>,
) -> Result<Response, WebError> {
    let raw = clean(form.status.as_deref()).unwrap_or_else(|| "not_run".to_string());
    let status = RunStatus::from_str(&raw)
        .map_err(|_| WebError::bad_request(format!("unknown step status: {raw}")))?;
    let duration = clean(form.duration_ms.as_deref())
        .and_then(|value| value.trim_end_matches("ms").trim().parse::<i64>().ok());
    let conn = db(&state)?;
    if repo::get_run(&conn, &id)?.is_none() {
        return Err(WebError::not_found(format!("run {id} not found")));
    }
    repo::record_step(
        &conn,
        &id,
        position,
        status,
        form.observed.as_deref().unwrap_or(""),
        form.notes.as_deref().unwrap_or(""),
        duration,
        clean(form.driver_test.as_deref()).as_deref(),
        form.raw_output.as_deref().unwrap_or(""),
    )?;
    Ok(redirect(
        &format!("/runs/{id}#step-{position}"),
        &format!("Step {position} recorded as {}.", status.as_str()),
    ))
}

pub async fn finish(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    let run = repo::finish_run(&conn, &id)?;
    Ok(redirect(
        &format!("/runs/{id}"),
        &format!("Run {id} finished as {}.", run.status.as_str()),
    ))
}

pub async fn upload_step_evidence(
    State(state): State<AppState>,
    Path((id, position)): Path<(String, i64)>,
    multipart: Multipart,
) -> Result<Response, WebError> {
    let upload = collect_multipart(multipart).await?;
    {
        let conn = db(&state)?;
        if repo::get_run(&conn, &id)?.is_none() {
            return Err(WebError::not_found(format!("run {id} not found")));
        }
        let Some(step_run_id) = data::step_run_id(&conn, &id, position)? else {
            return Err(WebError::not_found(format!(
                "step {position} of run {id} not found"
            )));
        };
        let role = clean_or(Some(upload.role.clone()), "actual");
        with_upload(&state, &upload, |path| {
            data::attach_evidence(
                &conn,
                &state.cfg.data_dir,
                EvidenceTarget::StepRun(step_run_id),
                path,
                &upload.caption,
                &role,
            )
        })?;
    }
    Ok(redirect(
        &format!("/runs/{id}#step-{position}"),
        &format!("Evidence attached to step {position}."),
    ))
}

pub async fn upload_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Response, WebError> {
    let upload = collect_multipart(multipart).await?;
    {
        let conn = db(&state)?;
        if repo::get_run(&conn, &id)?.is_none() {
            return Err(WebError::not_found(format!("run {id} not found")));
        }
        let role = clean_or(Some(upload.role.clone()), "actual");
        with_upload(&state, &upload, |path| {
            data::attach_evidence(
                &conn,
                &state.cfg.data_dir,
                EvidenceTarget::Run(id.clone()),
                path,
                &upload.caption,
                &role,
            )
        })?;
    }
    Ok(redirect(
        &format!("/runs/{id}#evidence"),
        "Evidence attached to the run.",
    ))
}
