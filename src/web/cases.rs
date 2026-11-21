//! Case catalogue, forms, steps, automation and expected artifacts.

use super::data::{self, CaseQuery, EvidenceTarget};
use super::{
    AppState, WebError, clean, clean_or, collect_multipart, db, flash_param, json_value, redirect,
    render, with_upload,
};
use crate::model::{RunMode, RunStatus, StepKind, TestCase, TestOutcome, TestRun};
use crate::repo;
use axum::extract::{Form, Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::str::FromStr;

#[derive(Debug, Deserialize, Default)]
pub struct ListParams {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub flash: Option<String>,
}

impl ListParams {
    fn filter(&self) -> CaseQuery {
        CaseQuery {
            q: clean(self.q.as_deref()),
            tag: clean(self.tag.as_deref()),
            category: clean(self.category.as_deref()),
            priority: clean(self.priority.as_deref()),
            status: clean(self.status.as_deref()),
        }
    }

    fn query_string(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in [
            ("q", &self.q),
            ("tag", &self.tag),
            ("category", &self.category),
            ("priority", &self.priority),
            ("status", &self.status),
        ] {
            if let Some(value) = clean(value.as_deref()) {
                parts.push(format!("{key}={}", urlencoding::encode(&value)));
            }
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let filter = params.filter();
    let cases = data::list_cases_filtered(&conn, &filter)?;
    let tags = data::tag_counts(&conn)?;
    let latest = data::latest_runs(&conn)?;

    let mut rows = Vec::with_capacity(cases.len());
    let mut automated = 0_i64;
    for case in &cases {
        let automation = data::case_automation(&conn, &case.id)?;
        if automation.automated {
            automated += 1;
        }
        let mut value = json_value(case)?;
        if let Value::Object(map) = &mut value {
            map.insert("automation".to_string(), json_value(&automation)?);
            let last = latest
                .get(&case.id)
                .map(|(status, started)| json!({ "status": status, "started_at": started }));
            map.insert("last_run".to_string(), last.unwrap_or(Value::Null));
        }
        rows.push(value);
    }

    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for tag in tags.iter().filter(|tag| tag.count > 0) {
        groups
            .entry(tag.category.clone())
            .or_default()
            .push(json_value(tag)?);
    }
    let order = ["level", "type", "technique", "quality", "custom"];
    let mut tag_groups = Vec::new();
    for category in order {
        if let Some(values) = groups.remove(category) {
            tag_groups.push(json!({ "name": category, "tags": values }));
        }
    }
    for (category, values) in groups {
        tag_groups.push(json!({ "name": category, "tags": values }));
    }

    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "cases.html",
        "cases",
        flash.as_deref(),
        json!({
            "cases": rows,
            "total": cases.len(),
            "automated": automated,
            "manual": cases.len() as i64 - automated,
            "tags": tags,
            "tag_groups": tag_groups,
            "filters": {
                "q": params.q, "tag": params.tag, "category": params.category,
                "priority": params.priority, "status": params.status
            },
            "filters_active": filter.is_active(),
            "query_string": params.query_string(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct CaseForm {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub module: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub environment: Option<String>,
    pub gherkin: Option<String>,
    pub tags: Option<String>,
    pub default_mode: Option<String>,
    pub driver_target: Option<String>,
}

fn parse_tags(raw: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for part in raw.split([',', ';']) {
        let tag = part.trim();
        if !tag.is_empty() && !tags.iter().any(|item: &String| item == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
}

fn valid_mode(raw: &str) -> bool {
    matches!(raw, "manual" | "single" | "composed")
}

fn valid_status(raw: &str) -> bool {
    matches!(raw, "draft" | "active" | "archived")
}

fn valid_priority(raw: &str) -> bool {
    matches!(raw, "low" | "medium" | "high" | "critical")
}

fn valid_case_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn values_from_form(form: &CaseForm) -> Value {
    json!({
        "id": form.id.clone().unwrap_or_default(),
        "title": form.title.clone().unwrap_or_default(),
        "description": form.description.clone().unwrap_or_default(),
        "module": form.module.clone().unwrap_or_default(),
        "priority": form.priority.clone().unwrap_or_else(|| "medium".into()),
        "status": form.status.clone().unwrap_or_else(|| "active".into()),
        "environment": form.environment.clone().unwrap_or_else(|| "local".into()),
        "gherkin": form.gherkin.clone().unwrap_or_default(),
        "tags": form.tags.clone().unwrap_or_default(),
        "default_mode": form.default_mode.clone().unwrap_or_else(|| "manual".into()),
        "driver_target": form.driver_target.clone().unwrap_or_default(),
    })
}

fn case_values(
    conn: &rusqlite::Connection,
    case: &crate::model::TestCase,
) -> Result<Value, WebError> {
    let automation = data::case_automation(conn, &case.id)?;
    Ok(json!({
        "id": case.id,
        "title": case.title,
        "description": case.description,
        "module": case.module,
        "priority": case.priority,
        "status": case.status,
        "environment": case.environment,
        "gherkin": case.gherkin,
        "tags": case.tags.join(", "),
        "default_mode": automation.default_mode,
        "driver_target": automation.driver_target.unwrap_or_default(),
    }))
}

fn form_context(
    conn: &rusqlite::Connection,
    editing: bool,
    values: Value,
    errors: Vec<String>,
) -> Result<Value, WebError> {
    let driver_tests = repo::list_driver_tests(conn, None)?;
    let tags = data::tag_counts(conn)?;
    Ok(json!({
        "editing": editing,
        "values": values,
        "errors": errors,
        "driver_tests": driver_tests,
        "tag_names": tags.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>(),
        "tag_suggestions": tags.iter().filter(|tag| tag.count > 0).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize, Default)]
pub struct NewParams {
    pub from_bug: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub flash: Option<String>,
}

pub async fn new_form(
    State(state): State<AppState>,
    Query(params): Query<NewParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let mut values = json!({
        "id": "", "title": "", "description": "", "module": "", "priority": "medium",
        "status": "active", "environment": "local", "gherkin": "", "tags": "",
        "default_mode": "manual", "driver_target": ""
    });
    if let Some(bug_id) = clean(params.from_bug.as_deref()) {
        let bug = repo::get_bug(&conn, &bug_id)?
            .ok_or_else(|| WebError::not_found(format!("bug {bug_id} not found")))?;
        let mut description = bug.description.clone();
        if !bug.steps.trim().is_empty() {
            description.push_str("\n\nReproduction:\n");
            description.push_str(&bug.steps);
        }
        if let Value::Object(map) = &mut values {
            map.insert("title".into(), Value::String(bug.title.clone()));
            map.insert("description".into(), Value::String(description));
            map.insert("tags".into(), Value::String("regression".to_string()));
        }
    } else {
        if let Value::Object(map) = &mut values {
            if let Some(title) = clean(params.title.as_deref()) {
                map.insert("title".into(), Value::String(title));
            }
            if let Some(description) = clean(params.description.as_deref()) {
                map.insert("description".into(), Value::String(description));
            }
        }
    }
    let context = form_context(&conn, false, values, Vec::new())?;
    let flash = flash_param(params.flash.as_deref());
    render(&state, "case_form.html", "cases", flash.as_deref(), context)
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<CaseForm>,
) -> Result<Response, WebError> {
    let id = form.id.as_deref().map(str::trim).unwrap_or("").to_string();
    let title = form
        .title
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let priority = clean_or(form.priority.clone(), "medium");
    let status = clean_or(form.status.clone(), "active");
    let mode = clean_or(form.default_mode.clone(), "manual");
    let mut errors = Vec::new();
    if !valid_case_id(&id) {
        errors
            .push("ID is required: letters, digits, dash, underscore or dot (max 64).".to_string());
    }
    if title.is_empty() {
        errors.push("Title is required.".to_string());
    }
    if !valid_priority(&priority) {
        errors.push("Priority must be low, medium, high or critical.".to_string());
    }
    if !valid_status(&status) {
        errors.push("Status must be draft, active or archived.".to_string());
    }
    if !valid_mode(&mode) {
        errors.push("Default mode must be manual, single or composed.".to_string());
    }
    let conn = db(&state)?;
    if errors.is_empty() && repo::get_case(&conn, &id)?.is_some() {
        errors.push(format!(
            "Case {id} already exists. Pick another ID or edit the existing case."
        ));
    }
    if !errors.is_empty() {
        let context = form_context(&conn, false, values_from_form(&form), errors)?;
        let response = render(&state, "case_form.html", "cases", None, context)?;
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, response).into_response());
    }

    let tags = parse_tags(form.tags.as_deref().unwrap_or(""));
    let case = repo::create_case(
        &conn,
        &id,
        &title,
        form.description.as_deref().unwrap_or(""),
        form.module.as_deref().unwrap_or(""),
        &priority,
        &clean_or(form.environment.clone(), "local"),
        form.gherkin.as_deref().unwrap_or(""),
        &tags,
    )?;
    if status != "active" {
        conn.execute(
            "UPDATE test_case SET status=?2,updated_at=?3 WHERE id=?1",
            params![case.id, status, crate::model::now()],
        )?;
    }
    let persisted =
        data::set_case_automation(&conn, &case.id, &mode, form.driver_target.as_deref())?;
    let flash = if persisted {
        format!("Case {} created.", case.id)
    } else {
        format!(
            "Case {} created. Automation fields wait for migration 0003.",
            case.id
        )
    };
    Ok(redirect(&format!("/cases/{}", case.id), &flash))
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let case = repo::get_case(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("case {id} not found")))?;
    let values = case_values(&conn, &case)?;
    let context = form_context(&conn, true, values, Vec::new())?;
    let flash = flash_param(params.flash.as_deref());
    render(&state, "case_form.html", "cases", flash.as_deref(), context)
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<CaseForm>,
) -> Result<Response, WebError> {
    let title = form
        .title
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let priority = clean_or(form.priority.clone(), "medium");
    let status = clean_or(form.status.clone(), "active");
    let mode = clean_or(form.default_mode.clone(), "manual");
    let mut errors = Vec::new();
    if title.is_empty() {
        errors.push("Title is required.".to_string());
    }
    if !valid_priority(&priority) {
        errors.push("Priority must be low, medium, high or critical.".to_string());
    }
    if !valid_status(&status) {
        errors.push("Status must be draft, active or archived.".to_string());
    }
    if !valid_mode(&mode) {
        errors.push("Default mode must be manual, single or composed.".to_string());
    }
    let conn = db(&state)?;
    let mut case = repo::get_case(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("case {id} not found")))?;
    if !errors.is_empty() {
        let context = form_context(&conn, true, values_from_form(&form), errors)?;
        let response = render(&state, "case_form.html", "cases", None, context)?;
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, response).into_response());
    }

    case.title = title;
    case.description = form.description.as_deref().unwrap_or("").to_string();
    case.module = form.module.as_deref().unwrap_or("").to_string();
    case.priority = priority;
    case.status = status;
    case.environment = clean_or(form.environment.clone(), "local");
    case.gherkin = form.gherkin.as_deref().unwrap_or("").to_string();
    case.tags = parse_tags(form.tags.as_deref().unwrap_or(""));
    repo::update_case(&conn, &case)?;
    let persisted = data::set_case_automation(&conn, &id, &mode, form.driver_target.as_deref())?;
    let flash = if persisted {
        format!("Case {id} saved.")
    } else {
        format!("Case {id} saved. Automation fields wait for migration 0003.")
    };
    Ok(redirect(&format!("/cases/{id}"), &flash))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    let removed = repo::delete_case(&conn, &id)?;
    let flash = if removed {
        format!("Case {id} deleted.")
    } else {
        format!("Case {id} was already gone.")
    };
    Ok(redirect("/cases", &flash))
}

#[derive(Debug, Deserialize, Default)]
pub struct RunForm {
    pub mode: Option<String>,
    pub environment: Option<String>,
}

pub async fn start_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<RunForm>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    let case = repo::get_case(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("case {id} not found")))?;
    let automation = data::case_automation(&conn, &id)?;
    let mode_raw = clean(form.mode.as_deref()).unwrap_or(automation.default_mode);
    let mode = RunMode::from_str(&mode_raw)
        .map_err(|_| WebError::bad_request(format!("unknown run mode: {mode_raw}")))?;
    let environment = clean(form.environment.as_deref())
        .or_else(|| clean(Some(case.environment.as_str())))
        .unwrap_or_else(|| "local".to_string());
    let run = repo::create_run(&conn, &id, mode, &environment)?;
    Ok(redirect(
        &format!("/runs/{}", run.id),
        &format!("Run {} started for {}.", run.id, id),
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct AutomateForm {
    pub default_mode: Option<String>,
    pub driver_target: Option<String>,
}

pub async fn automate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<AutomateForm>,
) -> Result<Response, WebError> {
    let mode = clean_or(form.default_mode.clone(), "manual");
    if !valid_mode(&mode) {
        return Err(WebError::bad_request(format!(
            "unknown mode {mode}; expected manual, single or composed"
        )));
    }
    let target = clean(form.driver_target.as_deref());
    if mode != "manual" && target.is_none() {
        return Err(WebError::bad_request(
            "Pick a discovered test target before switching to single or composed mode.",
        ));
    }
    let conn = db(&state)?;
    if repo::get_case(&conn, &id)?.is_none() {
        return Err(WebError::not_found(format!("case {id} not found")));
    }
    let persisted = data::set_case_automation(&conn, &id, &mode, target.as_deref())?;
    let flash = if persisted {
        match &target {
            Some(target) => format!("{id} now runs {mode} with target {target}."),
            None => format!("{id} now runs in {mode} mode."),
        }
    } else {
        "Migration 0003 is not applied yet; automation was not persisted.".to_string()
    };
    Ok(redirect(&format!("/cases/{id}#automation"), &flash))
}

#[derive(Debug, Deserialize, Default)]
pub struct StepForm {
    pub action: Option<String>,
    pub data: Option<String>,
    pub expected: Option<String>,
    pub kind: Option<String>,
    pub driver_target: Option<String>,
    pub direction: Option<String>,
}

fn step_kind(raw: Option<&str>) -> Result<StepKind, WebError> {
    match raw.map(str::trim).unwrap_or("manual") {
        "manual" => Ok(StepKind::Manual),
        "driver" => Ok(StepKind::Driver),
        other => Err(WebError::bad_request(format!(
            "step kind must be manual or driver, got {other}"
        ))),
    }
}

pub async fn add_step(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<StepForm>,
) -> Result<Response, WebError> {
    let action = form.action.as_deref().map(str::trim).unwrap_or("");
    if action.is_empty() {
        return Err(WebError::bad_request("A step needs an action."));
    }
    let kind = step_kind(form.kind.as_deref())?;
    let conn = db(&state)?;
    if repo::get_case(&conn, &id)?.is_none() {
        return Err(WebError::not_found(format!("case {id} not found")));
    }
    let position = data::next_position(&conn, &id)?;
    repo::add_step(
        &conn,
        &id,
        position,
        action,
        form.data.as_deref().unwrap_or(""),
        form.expected.as_deref().unwrap_or(""),
        kind,
        clean(form.driver_target.as_deref()).as_deref(),
    )?;
    Ok(redirect(
        &format!("/cases/{id}#steps"),
        &format!("Step {position} added to {id}."),
    ))
}

pub async fn update_step(
    State(state): State<AppState>,
    Path((id, position)): Path<(String, i64)>,
    Form(form): Form<StepForm>,
) -> Result<Response, WebError> {
    let action = form.action.as_deref().map(str::trim).unwrap_or("");
    if action.is_empty() {
        return Err(WebError::bad_request("A step needs an action."));
    }
    let kind = step_kind(form.kind.as_deref())?;
    let conn = db(&state)?;
    let changed = data::update_step(
        &conn,
        &id,
        position,
        action,
        form.data.as_deref().unwrap_or(""),
        form.expected.as_deref().unwrap_or(""),
        kind.as_str(),
        clean(form.driver_target.as_deref()).as_deref(),
    )?;
    let flash = if changed == 0 {
        format!("Step {position} of {id} was not found.")
    } else {
        format!("Step {position} of {id} saved.")
    };
    Ok(redirect(&format!("/cases/{id}#steps"), &flash))
}

pub async fn delete_step(
    State(state): State<AppState>,
    Path((id, position)): Path<(String, i64)>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    data::delete_step(&conn, &id, position)?;
    Ok(redirect(
        &format!("/cases/{id}#steps"),
        &format!("Step {position} removed from {id}."),
    ))
}

pub async fn move_step(
    State(state): State<AppState>,
    Path((id, position)): Path<(String, i64)>,
    Form(form): Form<StepForm>,
) -> Result<Response, WebError> {
    let direction = clean(form.direction.as_deref()).unwrap_or_default();
    let conn = db(&state)?;
    let moved = data::move_step(&conn, &id, position, &direction)?;
    let flash = if moved {
        format!("Step {position} moved {direction}.")
    } else {
        "That step cannot move any further.".to_string()
    };
    Ok(redirect(&format!("/cases/{id}#steps"), &flash))
}

pub async fn upload_evidence(
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Response, WebError> {
    let upload = collect_multipart(multipart).await?;
    {
        let conn = db(&state)?;
        if repo::get_case(&conn, &id)?.is_none() {
            return Err(WebError::not_found(format!("case {id} not found")));
        }
        let role = if upload.role.is_empty() {
            "expected"
        } else {
            upload.role.as_str()
        };
        with_upload(&state, &upload, |path| {
            data::attach_evidence(
                &conn,
                &state.cfg.data_dir,
                EvidenceTarget::Case(id.clone()),
                path,
                &upload.caption,
                role,
            )
        })?;
    }
    Ok(redirect(
        &format!("/cases/{id}#evidence"),
        "Evidence attached to the case.",
    ))
}

pub async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let case = repo::get_case(&conn, &id)?
        .ok_or_else(|| WebError::not_found(format!("case {id} not found")))?;
    let automation = data::case_automation(&conn, &id)?;
    let driver_tests = repo::list_driver_tests(&conn, None)?;
    let evidence = data::evidence_for(&conn, &EvidenceTarget::Case(id.clone()))?;
    let runs = repo::list_runs(&conn, Some(&id), 20)?;
    let all_bugs = repo::list_bugs(&conn, None)?;
    let bug_ids: Vec<String> = all_bugs.iter().map(|bug| bug.id.clone()).collect();
    let bugs: Vec<_> = all_bugs
        .into_iter()
        .filter(|bug| bug.case_id.as_deref() == Some(id.as_str()))
        .collect();
    let run_ids: Vec<String> = repo::list_runs(&conn, None, 100)?
        .into_iter()
        .map(|run| run.id)
        .collect();
    let links = data::list_links(&conn, "case", &id)?;
    let tags = data::tag_counts(&conn)?;
    let case_json = json_value(&case)?;
    let flash = flash_param(params.flash.as_deref());
    let expected: Vec<_> = evidence
        .iter()
        .filter(|item| item.role == "expected")
        .collect();
    let other: Vec<_> = evidence
        .iter()
        .filter(|item| item.role != "expected")
        .collect();
    render(
        &state,
        "case_detail.html",
        "cases",
        flash.as_deref(),
        json!({
            "case": case_json,
            "automation": automation,
            "driver_tests": driver_tests,
            "expected_evidence": expected,
            "evidence": other,
            "runs": runs,
            "run_ids": run_ids,
            "bugs": bugs,
            "bug_ids": bug_ids,
            "links": links,
            "tag_names": tags.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>(),
            "case_ids": data::list_cases_filtered(&conn, &CaseQuery::default())?
                .into_iter().map(|case| case.id).collect::<Vec<_>>(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct BulkForm {
    pub action: Option<String>,
    pub ids: Option<String>,
    pub tag: Option<String>,
}
pub async fn bulk(
    State(state): State<AppState>,
    Form(form): Form<BulkForm>,
) -> Result<Response, WebError> {
    let ids: Vec<String> = form
        .ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if ids.is_empty() {
        return Ok(redirect("/cases", "Select at least one case first."));
    }
    let action = clean(form.action.as_deref()).unwrap_or_default();
    let tag = clean(form.tag.as_deref());
    let conn = db(&state)?;
    let mut changed = 0_i64;
    match action.as_str() {
        "archive" | "activate" => {
            let status = if action == "archive" {
                "archived"
            } else {
                "active"
            };
            for id in &ids {
                changed += conn.execute(
                    "UPDATE test_case SET status=?2,updated_at=?3 WHERE id=?1",
                    params![id, status, crate::model::now()],
                )? as i64;
            }
        }
        "tag" | "untag" => {
            let Some(tag) = tag else {
                return Ok(redirect("/cases", "Choose a tag for the bulk action."));
            };
            for id in &ids {
                let Some(case) = repo::get_case(&conn, id)? else {
                    continue;
                };
                let mut tags = case.tags.clone();
                if action == "tag" {
                    if !tags.iter().any(|item| item == &tag) {
                        tags.push(tag.clone());
                    }
                } else {
                    tags.retain(|item| item != &tag);
                }
                repo::set_tags(&conn, id, &tags)?;
                changed += 1;
            }
        }
        "run" => {
            for id in &ids {
                if repo::get_case(&conn, id)?.is_none() {
                    continue;
                }
                let automation = data::case_automation(&conn, id)?;
                let mode = RunMode::from_str(&automation.default_mode).unwrap_or(RunMode::Manual);
                repo::create_run(&conn, id, mode, "local")?;
                changed += 1;
            }
        }
        other => {
            return Ok(redirect("/cases", &format!("Unknown bulk action: {other}")));
        }
    }
    Ok(redirect(
        "/cases",
        &format!("{changed} case(s) updated: {action}."),
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct RunFilteredForm {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchEntry {
    case_id: String,
    title: String,
    mode: String,
    outcome: String,
    run_id: Option<String>,
    message: String,
}

#[derive(Debug, Default, Serialize)]
struct BatchSummary {
    selected: usize,
    attempted: usize,
    created: usize,
    passed: usize,
    failed: usize,
    manual_required: usize,
    errors: usize,
    entries: Vec<BatchEntry>,
}

impl BatchSummary {
    fn push(
        &mut self,
        case: &TestCase,
        mode: RunMode,
        outcome: &str,
        run_id: Option<&str>,
        message: &str,
    ) {
        self.entries.push(BatchEntry {
            case_id: case.id.clone(),
            title: case.title.clone(),
            mode: mode.as_str().to_string(),
            outcome: outcome.to_string(),
            run_id: run_id.map(str::to_string),
            message: message.to_string(),
        });
    }
}

/// Run every case matching the current catalogue filters.
///
/// Mirrors `laburo run batch`: `auto` uses each case's default mode, manual
/// cases are skipped in auto mode, single/composed cases without a driver
/// target are counted as manual-required, and one case failure never aborts
/// the batch.
pub async fn run_filtered(
    State(state): State<AppState>,
    Form(form): Form<RunFilteredForm>,
) -> Result<Html<String>, WebError> {
    let requested = clean_or(form.mode.clone(), "auto");
    if !matches!(
        requested.as_str(),
        "auto" | "manual" | "single" | "composed"
    ) {
        return Err(WebError::bad_request(format!(
            "mode must be auto, manual, single or composed: {requested}"
        )));
    }
    let filter = CaseQuery {
        q: clean(form.q.as_deref()),
        tag: clean(form.tag.as_deref()),
        category: clean(form.category.as_deref()),
        priority: clean(form.priority.as_deref()),
        status: clean(form.status.as_deref()),
    };
    let cases = {
        let conn = db(&state)?;
        data::list_cases_filtered(&conn, &filter)?
    };
    let mut summary = BatchSummary {
        selected: cases.len(),
        ..BatchSummary::default()
    };
    for case in cases {
        summary.attempted += 1;
        let mode = if requested == "auto" {
            case.default_mode
        } else {
            RunMode::from_str(&requested).unwrap_or(RunMode::Manual)
        };
        if mode == RunMode::Manual && requested == "auto" {
            summary.manual_required += 1;
            summary.push(
                &case,
                mode,
                "manual-required",
                None,
                "Manual case in auto mode: start it from the case page when a person can run it.",
            );
            continue;
        }
        let targets = batch_targets(&case, mode);
        if mode != RunMode::Manual && targets.is_empty() {
            summary.manual_required += 1;
            summary.push(
                &case,
                mode,
                "manual-required",
                None,
                "No case-level driver target or driver step for this mode; attach a discovered test first.",
            );
            continue;
        }
        let run = {
            let conn = db(&state)?;
            repo::create_run(&conn, &case.id, mode, &case.environment)
        };
        let run = match run {
            Ok(run) => run,
            Err(error) => {
                summary.errors += 1;
                summary.push(
                    &case,
                    mode,
                    "error",
                    None,
                    &format!("Could not create the run: {error:#}"),
                );
                continue;
            }
        };
        summary.created += 1;
        if mode == RunMode::Manual {
            summary.push(
                &case,
                mode,
                "created",
                Some(&run.id),
                "Manual run created. Record each step from the run sheet.",
            );
            continue;
        }

        let root = state.cfg.root.clone();
        let driver_targets = targets;
        let result: anyhow::Result<Vec<TestOutcome>> =
            match tokio::task::spawn_blocking(move || {
                crate::driver::run_minitest(&root, &driver_targets)
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(anyhow::anyhow!("driver task failed: {error}")),
            };
        let conn = db(&state)?;
        match result {
            Ok(outcomes) if outcomes.is_empty() => {
                let message = "Minitest driver returned no outcomes";
                record_run_error(&conn, &run, message);
                let finished = repo::finish_run(&conn, &run.id)?;
                summary.errors += 1;
                summary.push(&case, mode, "error", Some(&finished.id), message);
            }
            Ok(outcomes) => {
                let mut recording_error = None;
                if let Err(error) = record_outcomes(&conn, &run, &outcomes) {
                    let message = format!("Could not record outcomes: {error:#}");
                    record_run_error(&conn, &run, &message);
                    summary.errors += 1;
                    recording_error = Some(message);
                }
                let finished = repo::finish_run(&conn, &run.id)?;
                if recording_error.is_none() {
                    match finished.status {
                        RunStatus::Passed => summary.passed += 1,
                        RunStatus::Failed | RunStatus::Error => summary.failed += 1,
                        _ => {}
                    }
                }
                summary.push(
                    &case,
                    mode,
                    finished.status.as_str(),
                    Some(&finished.id),
                    &recording_error
                        .unwrap_or_else(|| format!("{} step outcome(s) recorded.", outcomes.len())),
                );
            }
            Err(error) => {
                let message = format!("Minitest driver failed: {error:#}");
                record_run_error(&conn, &run, &message);
                let finished = repo::finish_run(&conn, &run.id)?;
                summary.errors += 1;
                summary.push(&case, mode, "error", Some(&finished.id), &message);
            }
        }
    }

    render(
        &state,
        "batch.html",
        "cases",
        None,
        json!({
            "summary": json_value(&summary)?,
            "requested_mode": requested,
            "filters": {
                "q": form.q, "tag": form.tag, "category": form.category,
                "priority": form.priority, "status": form.status
            },
            "filters_active": filter.is_active(),
        }),
    )
}

fn batch_targets(case: &TestCase, mode: RunMode) -> Vec<String> {
    match mode {
        RunMode::Manual => Vec::new(),
        RunMode::Single => case.driver_target.iter().cloned().collect(),
        RunMode::Composed => {
            let mut targets: Vec<String> = case
                .steps
                .iter()
                .filter_map(|step| step.driver_target.clone())
                .collect();
            if targets.is_empty() {
                targets.extend(case.driver_target.iter().cloned());
            }
            targets
        }
    }
}

fn record_outcomes(
    conn: &rusqlite::Connection,
    run: &TestRun,
    outcomes: &[TestOutcome],
) -> anyhow::Result<()> {
    for (index, outcome) in outcomes.iter().enumerate() {
        let position = (index + 1) as i64;
        let driver_test = run
            .steps
            .iter()
            .find(|step| step.position == position)
            .and_then(|step| step.driver_test.clone())
            .unwrap_or_else(|| outcome.target.clone());
        repo::record_step(
            conn,
            &run.id,
            position,
            outcome.status,
            &outcome.message,
            &outcome.backtrace,
            outcome.duration_ms,
            Some(&driver_test),
            &outcome.raw_output,
        )?;
    }
    Ok(())
}

fn record_run_error(conn: &rusqlite::Connection, run: &TestRun, message: &str) {
    for step in &run.steps {
        let _ = repo::record_step(
            conn,
            &run.id,
            step.position,
            RunStatus::Error,
            message,
            "",
            None,
            step.driver_test.as_deref(),
            message,
        );
    }
}

pub async fn export_csv(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    let cases = data::list_cases_filtered(&conn, &params.filter())?;
    let mut output = String::from(
        "id,title,module,priority,status,environment,default_mode,driver_target,tags,steps\n",
    );
    for case in &cases {
        let automation = data::case_automation(&conn, &case.id)?;
        output.push_str(&csv(&case.id));
        output.push(',');
        output.push_str(&csv(&case.title));
        output.push(',');
        output.push_str(&csv(&case.module));
        output.push(',');
        output.push_str(&csv(&case.priority));
        output.push(',');
        output.push_str(&csv(&case.status));
        output.push(',');
        output.push_str(&csv(&case.environment));
        output.push(',');
        output.push_str(&csv(&automation.default_mode));
        output.push(',');
        output.push_str(&csv(automation.driver_target.as_deref().unwrap_or("")));
        output.push(',');
        output.push_str(&csv(&case.tags.join(" ")));
        output.push(',');
        output.push_str(&case.steps.len().to_string());
        output.push('\n');
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"laburo-cases.csv\"".to_string(),
            ),
        ],
        output,
    )
        .into_response())
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
