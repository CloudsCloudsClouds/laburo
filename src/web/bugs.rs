//! Bug list, reproduction form, bug sheet and evidence.

use super::data::{self, EvidenceTarget};
use super::{
    AppState, WebError, clean, clean_or, collect_multipart, db, flash_param, json_value, redirect,
    render, with_upload,
};
use crate::model::BugReport;
use crate::repo;
use axum::extract::{Form, Multipart, Path, Query, State};
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub status: Option<String>,
    pub flash: Option<String>,
}

fn matches_query(bug: &BugReport, query: &str) -> bool {
    let needle = query.to_lowercase();
    bug.id.to_lowercase().contains(&needle)
        || bug.title.to_lowercase().contains(&needle)
        || bug
            .case_id
            .as_deref()
            .is_some_and(|id| id.to_lowercase().contains(&needle))
        || bug
            .run_id
            .as_deref()
            .is_some_and(|id| id.to_lowercase().contains(&needle))
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let status = clean(params.status.as_deref());
    let bugs = repo::list_bugs(&conn, status.as_deref())?;
    let query = clean(params.q.as_deref());
    let rows: Vec<_> = bugs
        .iter()
        .filter(|bug| {
            query
                .as_deref()
                .is_none_or(|query| matches_query(bug, query))
        })
        .map(json_value)
        .collect::<Result<Vec<_>, _>>()?;
    let all = repo::list_bugs(&conn, None)?;
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    for bug in &all {
        *counts.entry(bug.status.clone()).or_default() += 1;
    }
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "bugs.html",
        "bugs",
        flash.as_deref(),
        json!({
            "bugs": rows,
            "total": all.len(),
            "shown": rows.len(),
            "counts": counts,
            "filters": { "q": params.q, "status": params.status },
            "filters_active": query.is_some() || status.is_some(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct NewParams {
    pub case_id: Option<String>,
    pub run_id: Option<String>,
    pub step: Option<String>,
    pub observed: Option<String>,
    pub expected: Option<String>,
    pub title: Option<String>,
    pub flash: Option<String>,
}

fn prefill(
    conn: &rusqlite::Connection,
    params: &NewParams,
) -> Result<(Value, Option<String>, Option<String>), WebError> {
    let case_id = clean(params.case_id.as_deref());
    let run_id = clean(params.run_id.as_deref());
    let step_position = clean(params.step.as_deref()).and_then(|value| value.parse::<i64>().ok());
    let mut case_step = None;
    let mut step_run_id = None;
    let mut environment = "local".to_string();
    let mut actual = clean(params.observed.as_deref()).unwrap_or_default();
    let mut expected = clean(params.expected.as_deref()).unwrap_or_default();

    if let (Some(run_id), Some(position)) = (run_id.as_deref(), step_position) {
        if let Some(run) = repo::get_run(conn, run_id)? {
            environment = run.environment.clone();
            if let Some(step) = run.steps.iter().find(|step| step.position == position) {
                step_run_id = Some(step.id);
                if actual.is_empty() {
                    actual = step.observed.clone();
                }
            }
        }
    }
    if let Some(case_id) = case_id.as_deref()
        && let Some(case) = repo::get_case(conn, case_id)?
    {
        if let Some(position) = step_position
            && let Some(step) = case.steps.iter().find(|step| step.position == position)
        {
            case_step = Some(step.clone());
            if expected.is_empty() {
                expected = step.expected.clone();
            }
        }
        if run_id.is_none() {
            environment = case.environment.clone();
        }
    }

    let mut steps_text = String::new();
    if let Some(step) = &case_step {
        steps_text.push_str(&format!("1. {}\n", step.action));
        if !step.data.is_empty() {
            steps_text.push_str(&format!("   Data: {}\n", step.data));
        }
        steps_text.push_str("2. Observe the actual result.\n");
    } else if let Some(run_id) = run_id.as_deref() {
        steps_text.push_str(&format!("1. Open run {run_id}.\n"));
        if let Some(position) = step_position {
            steps_text.push_str(&format!("2. Replay step {position}.\n"));
        }
    }

    let title = clean(params.title.as_deref()).unwrap_or_else(|| match (&case_step, &run_id) {
        (_, Some(run_id)) if step_run_id.is_some() => {
            format!("Step {step_position:?} failed in {run_id}")
        }
        _ => String::new(),
    });

    Ok((
        json!({
            "id": "",
            "title": title,
            "description": "",
            "steps": steps_text,
            "actual": actual,
            "expected": expected,
            "priority": "2",
            "severity": "3",
            "environment": environment,
            "case_id": case_id.unwrap_or_default(),
            "run_id": run_id.unwrap_or_default(),
            "step_run_id": step_run_id.map(|id| id.to_string()).unwrap_or_default(),
        }),
        clean(params.case_id.as_deref()),
        clean(params.run_id.as_deref()),
    ))
}

fn form_context(
    conn: &rusqlite::Connection,
    values: Value,
    errors: Vec<String>,
    case_id: Option<&str>,
    run_id: Option<&str>,
) -> Result<Value, WebError> {
    let cases = repo::list_cases(conn, None, None)?;
    let runs = repo::list_runs(conn, None, 200)?;
    let bugs = repo::list_bugs(conn, None)?;
    Ok(json!({
        "values": values,
        "errors": errors,
        "case_ids": cases.into_iter().map(|case| case.id).collect::<Vec<_>>(),
        "run_ids": runs.into_iter().map(|run| run.id).collect::<Vec<_>>(),
        "bug_ids": bugs.into_iter().map(|bug| bug.id).collect::<Vec<_>>(),
        "has_case": case_id.is_some(),
        "has_run": run_id.is_some(),
    }))
}

pub async fn new_form(
    State(state): State<AppState>,
    Query(params): Query<NewParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let (values, case_id, run_id) = prefill(&conn, &params)?;
    let context = form_context(
        &conn,
        values,
        Vec::new(),
        case_id.as_deref(),
        run_id.as_deref(),
    )?;
    let flash = flash_param(params.flash.as_deref());
    render(&state, "bug_form.html", "bugs", flash.as_deref(), context)
}

#[derive(Debug, Deserialize, Default)]
pub struct BugForm {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub steps: Option<String>,
    pub actual: Option<String>,
    pub expected: Option<String>,
    pub priority: Option<String>,
    pub severity: Option<String>,
    pub environment: Option<String>,
    pub case_id: Option<String>,
    pub run_id: Option<String>,
    pub step_run_id: Option<String>,
    pub comments: Option<String>,
}

fn level(raw: Option<&str>) -> i64 {
    raw.and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| (1..=4).contains(value))
        .unwrap_or(3)
}

fn values_from_form(form: &BugForm) -> Value {
    json!({
        "id": form.id.clone().unwrap_or_default(),
        "title": form.title.clone().unwrap_or_default(),
        "description": form.description.clone().unwrap_or_default(),
        "steps": form.steps.clone().unwrap_or_default(),
        "actual": form.actual.clone().unwrap_or_default(),
        "expected": form.expected.clone().unwrap_or_default(),
        "priority": form.priority.clone().unwrap_or_else(|| "3".into()),
        "severity": form.severity.clone().unwrap_or_else(|| "3".into()),
        "environment": form.environment.clone().unwrap_or_else(|| "local".into()),
        "case_id": form.case_id.clone().unwrap_or_default(),
        "run_id": form.run_id.clone().unwrap_or_default(),
        "step_run_id": form.step_run_id.clone().unwrap_or_default(),
    })
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<BugForm>,
) -> Result<Response, WebError> {
    let title = form
        .title
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let case_id = clean(form.case_id.as_deref());
    let run_id = clean(form.run_id.as_deref());
    let step_run_id =
        clean(form.step_run_id.as_deref()).and_then(|value| value.parse::<i64>().ok());
    let mut errors = Vec::new();
    if title.is_empty() {
        errors.push("Title is required.".to_string());
    }
    if let Some(id) = clean(form.id.as_deref())
        && (id.len() > 64
            || !id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            }))
    {
        errors
            .push("Bug ID may use letters, digits, dash, underscore or dot (max 64).".to_string());
    }
    let conn = db(&state)?;
    if let Some(id) = clean(form.id.as_deref())
        && repo::get_bug(&conn, &id)?.is_some()
    {
        errors.push(format!("Bug {id} already exists."));
    }
    if let Some(case_id) = case_id.as_deref()
        && repo::get_case(&conn, case_id)?.is_none()
    {
        errors.push(format!("Case {case_id} does not exist."));
    }
    if let Some(run_id) = run_id.as_deref()
        && repo::get_run(&conn, run_id)?.is_none()
    {
        errors.push(format!("Run {run_id} does not exist."));
    }
    if !errors.is_empty() {
        let context = form_context(
            &conn,
            values_from_form(&form),
            errors,
            case_id.as_deref(),
            run_id.as_deref(),
        )?;
        let response = render(&state, "bug_form.html", "bugs", None, context)?;
        return Ok((axum::http::StatusCode::UNPROCESSABLE_ENTITY, response).into_response());
    }

    let bug = repo::create_bug(
        &conn,
        clean(form.id.as_deref()).as_deref(),
        &title,
        form.description.as_deref().unwrap_or(""),
        form.steps.as_deref().unwrap_or(""),
        form.actual.as_deref().unwrap_or(""),
        form.expected.as_deref().unwrap_or(""),
        level(form.priority.as_deref()),
        level(form.severity.as_deref()),
        &clean_or(form.environment.clone(), "local"),
        case_id.as_deref(),
        run_id.as_deref(),
        step_run_id,
        form.comments.as_deref().unwrap_or(""),
    )?;
    let flash = if bug.case_id.is_none() {
        format!(
            "Bug {} filed. It has no reproducing case; open the bug to create one or link it.",
            bug.id
        )
    } else {
        format!("Bug {} filed.", bug.id)
    };
    Ok(redirect(&format!("/bugs/{}", bug.id), &flash))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let bug = repo::get_bug(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("bug {id} not found")))?;
    let case = match bug.case_id.as_deref() {
        Some(case_id) => repo::get_case(&conn, case_id)?,
        None => None,
    };
    let run = match bug.run_id.as_deref() {
        Some(run_id) => repo::get_run(&conn, run_id)?,
        None => None,
    };
    let step_run = match bug.step_run_id {
        Some(step_run_id) => conn
            .query_row(
                "SELECT run_id, position FROM step_run WHERE id=?1",
                [step_run_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .ok(),
        None => None,
    };
    let evidence = data::evidence_for(&conn, &EvidenceTarget::Bug(id.clone()))?;
    let links = data::list_links(&conn, "bug", &id)?;
    let cases = repo::list_cases(&conn, None, None)?;
    let runs = repo::list_runs(&conn, None, 200)?;
    let bugs = repo::list_bugs(&conn, None)?;
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "bug_detail.html",
        "bugs",
        flash.as_deref(),
        json!({
            "bug": json_value(&bug)?,
            "case": case.map(|case| json_value(&case)).transpose()?,
            "run": run.map(|run| json_value(&run)).transpose()?,
            "step_run": step_run.map(|(run_id, position)| json!({
                "run_id": run_id, "position": position
            })),
            "evidence": evidence,
            "links": links,
            "case_ids": cases.into_iter().map(|case| case.id).collect::<Vec<_>>(),
            "run_ids": runs.into_iter().map(|run| run.id).collect::<Vec<_>>(),
            "bug_ids": bugs.into_iter().map(|bug| bug.id).collect::<Vec<_>>(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct StatusForm {
    pub status: Option<String>,
}

pub async fn set_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<StatusForm>,
) -> Result<Response, WebError> {
    let status = clean(form.status.as_deref()).unwrap_or_default();
    if !matches!(status.as_str(), "open" | "resolved" | "closed" | "reopened") {
        return Err(WebError::bad_request(format!(
            "unknown bug status: {status}"
        )));
    }
    let conn = db(&state)?;
    if repo::get_bug(&conn, &id)?.is_none() {
        return Err(WebError::not_found(format!("bug {id} not found")));
    }
    repo::set_bug_status(&conn, &id, &status)?;
    Ok(redirect(
        &format!("/bugs/{id}"),
        &format!("Bug {id} is now {status}."),
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
        if repo::get_bug(&conn, &id)?.is_none() {
            return Err(WebError::not_found(format!("bug {id} not found")));
        }
        let role = clean_or(Some(upload.role.clone()), "evidence");
        with_upload(&state, &upload, |path| {
            data::attach_evidence(
                &conn,
                &state.cfg.data_dir,
                EvidenceTarget::Bug(id.clone()),
                path,
                &upload.caption,
                &role,
            )
        })?;
    }
    Ok(redirect(
        &format!("/bugs/{id}#evidence"),
        "Evidence attached to the bug.",
    ))
}
