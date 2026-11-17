use laburo::db::Db;
use laburo::model::{RunMode, RunStatus, StepKind};
use laburo::repo;

#[test]
fn manual_flow_case_run_bug_dashboard() {
    let db = Db::open_memory().unwrap();
    let case = repo::create_case(
        &db.conn,
        "TC-001",
        "Booking happy path",
        "A tourist books a tour",
        "M-02",
        "high",
        "local",
        "Feature: Booking\n  Scenario: happy path",
        &["integration".to_string(), "functional".to_string()],
    )
    .unwrap();
    assert_eq!(case.tags.len(), 2);
    assert_eq!(case.steps.len(), 0);

    repo::add_step(
        &db.conn,
        "TC-001",
        1,
        "Open the catalogue",
        "",
        "The catalogue is visible",
        StepKind::Manual,
        None,
    )
    .unwrap();
    repo::add_step(
        &db.conn,
        "TC-001",
        2,
        "Submit the booking",
        "Ana",
        "The booking is created",
        StepKind::Manual,
        None,
    )
    .unwrap();

    let case = repo::get_case(&db.conn, "TC-001").unwrap().unwrap();
    assert_eq!(case.steps.len(), 2);

    let run = repo::create_run(&db.conn, "TC-001", RunMode::Manual, "local").unwrap();
    assert_eq!(run.steps.len(), 2);

    repo::record_step(
        &db.conn,
        &run.id,
        1,
        RunStatus::Passed,
        "catalogue visible",
        "",
        Some(10),
        None,
        "",
    )
    .unwrap();
    repo::record_step(
        &db.conn,
        &run.id,
        2,
        RunStatus::Failed,
        "500 error",
        "backend failed",
        Some(30),
        None,
        "trace",
    )
    .unwrap();

    let run = repo::finish_run(&db.conn, &run.id).unwrap();
    assert_eq!(run.status, RunStatus::Failed);

    let bug = repo::create_bug(
        &db.conn,
        None,
        "Booking returns 500",
        "The booking endpoint fails",
        "1. Open catalogue\n2. Submit",
        "500 error",
        "Booking created",
        1,
        1,
        "local",
        Some("TC-001"),
        Some(&run.id),
        run.steps.get(1).map(|s| s.id),
        "seen twice",
    )
    .unwrap();
    assert_eq!(bug.case_id.as_deref(), Some("TC-001"));
    assert_eq!(bug.status, "open");

    let dash = repo::dashboard(&db.conn).unwrap();
    assert_eq!(dash.cases, 1);
    assert_eq!(dash.runs, 1);
    assert_eq!(dash.failed_runs, 1);
    assert_eq!(dash.bugs_open, 1);
}

#[test]
fn evidence_is_copied_and_hashed() {
    let db = Db::open_memory().unwrap();
    repo::create_case(
        &db.conn,
        "TC-002",
        "Evidence",
        "",
        "",
        "low",
        "local",
        "",
        &[],
    )
    .unwrap();
    let run = repo::create_run(&db.conn, "TC-002", RunMode::Manual, "local").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("shot.png");
    std::fs::write(&src, b"not really a png").unwrap();
    let step_id = run.steps[0].id;
    let att = repo::attach_file(
        &db.conn,
        dir.path(),
        repo::EvidenceOwner::StepRun(step_id),
        &src,
        "screenshot",
    )
    .unwrap();
    assert_eq!(att.kind, "screenshot");
    assert_eq!(att.original_name, "shot.png");
    assert!(!att.sha256.is_empty());
    assert!(dir.path().join(&att.path).exists(), "{}", att.path);
}
