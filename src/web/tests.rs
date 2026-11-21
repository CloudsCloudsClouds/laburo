//! Offline route smoke tests. No network, no Typst, no Rails.

use super::{AppState, router};
use crate::config::ProjectConfig;
use crate::db::Db;
use crate::model::{DiscoveredTest, RunMode, RunStatus, StepKind};
use crate::repo::{self, EvidenceOwner};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    _dir: TempDir,
    state: AppState,
    run_id: String,
    bug_id: String,
}

fn seed(conn: &rusqlite::Connection, data_dir: &Path) -> (String, String) {
    repo::create_case(
        conn,
        "TC-001",
        "Book a tour",
        "A visitor can book a tour with two travellers.",
        "Bookings",
        "high",
        "local",
        "Feature: Booking",
        &[
            "smoke".to_string(),
            "functional".to_string(),
            "system".to_string(),
        ],
    )
    .unwrap();
    repo::add_step(
        conn,
        "TC-001",
        1,
        "Open the tour page",
        "",
        "The tour page is visible",
        StepKind::Manual,
        None,
    )
    .unwrap();
    repo::add_step(
        conn,
        "TC-001",
        2,
        "Submit the booking",
        "travellers=2",
        "A confirmation with a reference is shown",
        StepKind::Manual,
        None,
    )
    .unwrap();
    repo::create_case(
        conn,
        "TC-002",
        "Admin pages reject anonymous users",
        "Unauthorized admin access is denied.",
        "Admin",
        "critical",
        "local",
        "",
        &["security".to_string(), "functional".to_string()],
    )
    .unwrap();
    repo::add_step(
        conn,
        "TC-002",
        1,
        "Open /admin without a session",
        "",
        "A 403 or login page is returned",
        StepKind::Manual,
        None,
    )
    .unwrap();

    let run = repo::create_run(conn, "TC-001", RunMode::Manual, "local").unwrap();
    repo::record_step(
        conn,
        &run.id,
        1,
        RunStatus::Passed,
        "Tour page rendered",
        "",
        Some(21),
        None,
        "",
    )
    .unwrap();
    repo::record_step(
        conn,
        &run.id,
        2,
        RunStatus::Failed,
        "HTTP 500 after submit",
        "reproduced twice",
        None,
        None,
        "Error: undefined method for nil",
    )
    .unwrap();
    repo::finish_run(conn, &run.id).unwrap();

    let bug = repo::create_bug(
        conn,
        Some("BUG-001"),
        "Booking confirmation returns 500",
        "Two travellers break the confirmation step.",
        "1. Open /tours\n2. Submit a booking for 2 travellers",
        "HTTP 500",
        "A confirmation with a reference",
        1,
        2,
        "local",
        Some("TC-001"),
        Some(&run.id),
        None,
        "",
    )
    .unwrap();

    std::fs::write(data_dir.join("run.log"), "line one\nline two\n").unwrap();
    repo::attach_file(
        conn,
        data_dir,
        EvidenceOwner::Run(run.id.clone()),
        &data_dir.join("run.log"),
        "server log",
    )
    .unwrap();

    repo::ensure_driver(conn, "minitest", "minitest", "Minitest").unwrap();
    repo::save_driver_tests(
        conn,
        "minitest",
        &[DiscoveredTest {
            id: "test/system/booking_test.rb#BookingTest#test_books".to_string(),
            driver_id: "minitest".to_string(),
            name: "BookingTest#test_books".to_string(),
            file: "test/system/booking_test.rb".to_string(),
            line: Some(7),
            metadata: json!({ "system": true, "kind": "def" }),
        }],
    )
    .unwrap();

    (run.id, bug.id)
}

fn fixture() -> Fixture {
    let db = Db::open_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().join(".laburo");
    std::fs::create_dir_all(data_dir.join("reports")).unwrap();
    std::fs::create_dir_all(data_dir.join("outbox")).unwrap();
    std::fs::write(
        data_dir.join("outbox").join("pre-existing.eml"),
        "From: Laburo <laburo@localhost>\nTo: <demo@example.edu>\nSubject: seeded eml\n\nBody\n",
    )
    .unwrap();
    let (run_id, bug_id) = seed(&db.conn, &data_dir);
    let cfg = ProjectConfig {
        root: dir.path().to_path_buf(),
        data_dir: data_dir.clone(),
        db_path: data_dir.join("laburo.sqlite3"),
        typst: "typst".to_string(),
    };
    let state = AppState::new(cfg, db.conn).unwrap();
    Fixture {
        _dir: dir,
        state,
        run_id,
        bug_id,
    }
}

async fn get(state: AppState, uri: &str) -> (StatusCode, String) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn post_form(state: AppState, uri: &str, body: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    (status, location)
}

async fn post_form_body(state: AppState, uri: &str, body: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router(state).oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn every_primary_page_renders() {
    let fixture = fixture();
    for uri in [
        "/",
        "/cases",
        "/cases/new",
        "/runs",
        "/bugs",
        "/bugs/new",
        "/drivers",
        "/reports",
        "/quality",
        "/email",
        "/search",
        "/search?q=booking",
    ] {
        let (status, body) = get(fixture.state.clone(), uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} failed: {body}");
        assert!(body.contains("Laburo"), "{uri} did not render the shell");
    }
    let (status, body) = get(fixture.state.clone(), "/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn detail_pages_render_seeded_records() {
    let fixture = fixture();
    let (status, body) = get(fixture.state.clone(), "/cases/TC-001").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Book a tour"));
    assert!(body.contains("A confirmation with a reference is shown"));

    let (status, body) = get(fixture.state.clone(), &format!("/runs/{}", fixture.run_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("HTTP 500 after submit"));
    assert!(body.contains("File bug"));

    let (status, body) = get(fixture.state.clone(), &format!("/bugs/{}", fixture.bug_id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Booking confirmation returns 500"));
    assert!(body.contains("TC-001"));
}

#[tokio::test]
async fn missing_records_return_404() {
    let fixture = fixture();
    let (status, _) = get(fixture.state.clone(), "/cases/TC-NOPE").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(fixture.state.clone(), "/runs/RUN-NOPE").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn case_creation_persists_and_redirects() {
    let fixture = fixture();
    let body = "id=TC-900&title=Create+a+booking&description=Smoke&module=Bookings&priority=high&status=active&environment=local&tags=smoke%2C+functional&default_mode=manual&driver_target=&gherkin=";
    let (status, location) = post_form(fixture.state.clone(), "/cases", body).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        location.starts_with("/cases/TC-900?flash="),
        "got {location}"
    );

    let conn = fixture.state.db.lock().unwrap();
    let case = repo::get_case(&conn, "TC-900")
        .unwrap()
        .expect("case created");
    assert_eq!(case.title, "Create a booking");
    assert_eq!(
        case.tags,
        vec!["functional".to_string(), "smoke".to_string()]
    );
    assert_eq!(case.steps.len(), 0);
}

#[tokio::test]
async fn invalid_case_form_renders_inline_errors() {
    let fixture = fixture();
    let body = "id=&title=&priority=high&status=active&environment=local";
    let request = Request::builder()
        .method("POST")
        .uri("/cases")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let response = router(fixture.state.clone())
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("ID is required"));
    assert!(text.contains("Title is required"));
}

#[tokio::test]
async fn run_all_filtered_creates_runs_for_the_filtered_tag() {
    let fixture = fixture();
    let (status, body) = post_form_body(
        fixture.state.clone(),
        "/cases/run-filtered",
        "tag=smoke&mode=manual",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Batch run summary"));
    assert!(body.contains("TC-001"));
    assert!(body.contains("created"));

    let conn = fixture.state.db.lock().unwrap();
    let runs = repo::list_runs(&conn, Some("TC-001"), 50).unwrap();
    assert_eq!(runs.len(), 2, "seed run plus batch run");
    assert!(runs.iter().any(|run| run.status == RunStatus::Running));
    let security_runs = repo::list_runs(&conn, Some("TC-002"), 50).unwrap();
    assert!(security_runs.is_empty(), "filter leaked into another tag");
}

#[tokio::test]
async fn run_all_filtered_counts_missing_targets_as_manual_required() {
    let fixture = fixture();
    let (status, body) = post_form_body(
        fixture.state.clone(),
        "/cases/run-filtered",
        "tag=security&mode=single",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("manual-required"));
    assert!(body.contains("TC-002"));
    let conn = fixture.state.db.lock().unwrap();
    assert!(
        repo::list_runs(&conn, Some("TC-002"), 50)
            .unwrap()
            .is_empty(),
        "a case without a driver target must not get an automated run"
    );
}

#[tokio::test]
async fn run_all_filtered_records_driver_errors_without_aborting() {
    let fixture = fixture();
    {
        let conn = fixture.state.db.lock().unwrap();
        repo::set_case_automation(
            &conn,
            "TC-001",
            crate::model::RunMode::Single,
            Some("test/system/booking_test.rb#BookingTest#test_books"),
        )
        .unwrap();
    }
    let (status, body) = post_form_body(
        fixture.state.clone(),
        "/cases/run-filtered",
        "tag=smoke&mode=single",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("error"));
    assert!(body.contains("Minitest driver failed"));
    let conn = fixture.state.db.lock().unwrap();
    let runs = repo::list_runs(&conn, Some("TC-001"), 50).unwrap();
    assert_eq!(runs.len(), 2);
    assert!(
        runs.iter()
            .any(|run| { run.status.is_bad() && run.mode == crate::model::RunMode::Single })
    );
}

#[tokio::test]
async fn cases_page_shows_run_all_filtered_form() {
    let fixture = fixture();
    let (status, body) = get(fixture.state.clone(), "/cases?tag=smoke").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Run all filtered"));
    assert!(body.contains("action=\"/cases/run-filtered\""));
    assert!(body.contains("name=\"tag\" value=\"smoke\""));
}

#[tokio::test]
async fn automation_badge_reflects_the_case_mode() {
    let fixture = fixture();
    let body =
        "default_mode=single&driver_target=test/system/booking_test.rb%23BookingTest%23test_books";
    let (status, _) = post_form(fixture.state.clone(), "/cases/TC-001/automate", body).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (status, body) = get(fixture.state.clone(), "/cases/TC-001").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("badge-single"), "single badge missing");
    assert!(
        !body.contains("manual-required"),
        "automated case still rendered as manual-required"
    );
}

#[tokio::test]
async fn email_page_lists_preexisting_outbox_files() {
    let fixture = fixture();
    let (status, body) = get(fixture.state.clone(), "/email").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("pre-existing.eml"), "outbox file not listed");
    assert!(
        body.contains("Download .eml"),
        "outbox has no download link"
    );
    assert!(body.contains("1 file(s)"), "outbox count is wrong");
}

#[tokio::test]
async fn reopens_the_database_after_the_file_is_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let data_dir = directory.path().join(".laburo");
    std::fs::create_dir_all(&data_dir).unwrap();
    let cfg = ProjectConfig {
        root: directory.path().to_path_buf(),
        data_dir: data_dir.clone(),
        db_path: data_dir.join("laburo.sqlite3"),
        typst: "typst".to_string(),
    };
    let db = Db::open(&cfg.db_path).unwrap();
    repo::create_case(
        &db.conn,
        "TC-OLD",
        "Old seed",
        "",
        "seed",
        "medium",
        "local",
        "",
        &[],
    )
    .unwrap();
    let state = AppState::new(cfg.clone(), db.conn).unwrap();
    let (_, body) = get(state.clone(), "/cases").await;
    assert!(body.contains("TC-OLD"));

    // Simulate demo-seed.sh: remove the whole .laburo data file, recreate it
    // with different records, while the server keeps its old connection.
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", cfg.db_path.display()));
    }
    let fresh = Db::open(&cfg.db_path).unwrap();
    repo::create_case(
        &fresh.conn,
        "TC-NEW",
        "Fresh seed",
        "",
        "seed",
        "medium",
        "local",
        "",
        &[],
    )
    .unwrap();
    drop(fresh.conn);

    let (status, body) = get(state, "/cases").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("TC-NEW"), "server did not reopen the new DB");
    assert!(!body.contains("TC-OLD"), "server kept serving the stale DB");
}

#[tokio::test]
async fn tag_and_text_filters_narrow_the_catalogue() {
    let fixture = fixture();
    let (status, body) = get(fixture.state.clone(), "/cases?tag=smoke").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("TC-001"));
    assert!(
        !body.contains("TC-002"),
        "security case leaked into smoke filter"
    );

    let (status, body) = get(
        fixture.state.clone(),
        "/cases?q=confirmation%20with%20a%20reference",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("TC-001"), "search did not reach step text");

    let (status, body) = get(fixture.state.clone(), "/cases?q=TC-002").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("TC-002"));
    assert!(!body.contains("TC-001"));
}

#[tokio::test]
async fn starting_a_run_creates_a_run_sheet() {
    let fixture = fixture();
    let body = "mode=manual&environment=local";
    let (status, location) = post_form(fixture.state.clone(), "/cases/TC-001/run", body).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(location.starts_with("/runs/RUN-"), "got {location}");
    let conn = fixture.state.db.lock().unwrap();
    let runs = repo::list_runs(&conn, Some("TC-001"), 50).unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().any(|run| run.status == RunStatus::Running));
}

#[tokio::test]
async fn run_evidence_upload_is_multipart_and_lands_in_the_run() {
    let fixture = fixture();
    let boundary = "----laburo-test-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nconsole log\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"role\"\r\n\r\nactual\r\n\
         --{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nhello evidence\r\n\
         --{boundary}--\r\n"
    );
    let request = Request::builder()
        .method("POST")
        .uri(format!("/runs/{}/evidence", fixture.run_id))
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    let response = router(fixture.state.clone())
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let conn = fixture.state.db.lock().unwrap();
    let evidence = repo::load_evidence(&conn, EvidenceOwner::Run(fixture.run_id.clone())).unwrap();
    assert_eq!(evidence.len(), 2, "seeded log plus upload");
}

#[tokio::test]
async fn static_assets_are_served_without_a_cdn() {
    let fixture = fixture();
    let (status, body) = get(fixture.state.clone(), "/static/app.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Windows 7-era") || body.contains("--accent"));
    let (status, body) = get(fixture.state.clone(), "/static/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("data-copy"));
}
