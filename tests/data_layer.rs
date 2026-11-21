use laburo::db::Db;
use laburo::model::{CaseFilter, RunMode, RunStatus, StepKind};
use laburo::repo;

fn case(db: &Db, id: &str, title: &str, priority: &str, tags: &[&str]) {
    repo::create_case(
        &db.conn,
        id,
        title,
        "",
        "M-01",
        priority,
        "local",
        "",
        &tags
            .iter()
            .map(|tag| (*tag).to_string())
            .collect::<Vec<_>>(),
    )
    .unwrap();
}

#[test]
fn list_cases_filtered_matches_text_tag_category_priority_and_status() {
    let db = Db::open_memory().unwrap();
    case(&db, "TC-001", "Book a tour", "high", &["smoke", "system"]);
    case(
        &db,
        "TC-002",
        "Reset a password",
        "low",
        &["unit", "security", "white-box"],
    );
    case(&db, "TC-003", "Archived tour", "high", &["smoke", "unit"]);
    let mut archived = repo::get_case(&db.conn, "TC-003").unwrap().unwrap();
    archived.status = "archived".to_string();
    repo::update_case(&db.conn, &archived).unwrap();

    let mut filter = CaseFilter {
        q: Some("password".to_string()),
        ..CaseFilter::default()
    };
    let found = repo::list_cases_filtered(&db.conn, &filter).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "TC-002");

    // The q filter also searches step action/expected text.
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
    filter.q = Some("catalogue".to_string());
    let found = repo::list_cases_filtered(&db.conn, &filter).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "TC-001");

    filter = CaseFilter {
        tag: Some("smoke".to_string()),
        ..CaseFilter::default()
    };
    let ids: Vec<String> = repo::list_cases_filtered(&db.conn, &filter)
        .unwrap()
        .into_iter()
        .map(|case| case.id)
        .collect();
    assert_eq!(ids, vec!["TC-001", "TC-003"]);

    filter = CaseFilter {
        category: Some("technique".to_string()),
        ..CaseFilter::default()
    };
    let ids: Vec<String> = repo::list_cases_filtered(&db.conn, &filter)
        .unwrap()
        .into_iter()
        .map(|case| case.id)
        .collect();
    assert_eq!(ids, vec!["TC-002"]);

    filter = CaseFilter {
        priority: Some("high".to_string()),
        status: Some("archived".to_string()),
        ..CaseFilter::default()
    };
    let ids: Vec<String> = repo::list_cases_filtered(&db.conn, &filter)
        .unwrap()
        .into_iter()
        .map(|case| case.id)
        .collect();
    assert_eq!(ids, vec!["TC-003"]);

    let counts = repo::tag_counts(&db.conn).unwrap();
    let smoke = counts
        .iter()
        .find(|(name, _, _)| name == "smoke")
        .expect("smoke tag");
    assert_eq!(smoke.1, "quality");
    assert_eq!(smoke.2, 2);
}

#[test]
fn set_case_automation_persists_mode_and_target() {
    let db = Db::open_memory().unwrap();
    case(&db, "TC-001", "Happy path", "high", &["smoke"]);
    let updated = repo::set_case_automation(
        &db.conn,
        "TC-001",
        RunMode::Single,
        Some("test/system/booking_test.rb#BookingTest#test_happy_path"),
    )
    .unwrap();
    assert_eq!(updated.default_mode, RunMode::Single);
    assert_eq!(
        updated.driver_target.as_deref(),
        Some("test/system/booking_test.rb#BookingTest#test_happy_path")
    );

    let mut changed = updated;
    changed.title = "Happy path updated".to_string();
    repo::update_case(&db.conn, &changed).unwrap();
    let reread = repo::get_case(&db.conn, "TC-001").unwrap().unwrap();
    assert_eq!(reread.default_mode, RunMode::Single);
    assert_eq!(reread.driver_target, changed.driver_target);

    let error = repo::set_case_automation(&db.conn, "TC-404", RunMode::Manual, None).unwrap_err();
    assert!(error.to_string().contains("case not found"));
}

#[test]
fn single_mode_creates_one_synthetic_step_and_can_pass() {
    let db = Db::open_memory().unwrap();
    case(&db, "TC-001", "Happy path", "high", &["smoke"]);
    for position in 1..=2 {
        repo::add_step(
            &db.conn,
            "TC-001",
            position,
            "step",
            "",
            "expected",
            StepKind::Manual,
            None,
        )
        .unwrap();
    }
    repo::set_case_automation(
        &db.conn,
        "TC-001",
        RunMode::Single,
        Some("test/system/booking_test.rb#BookingTest#test_happy_path"),
    )
    .unwrap();

    let run = repo::create_run(&db.conn, "TC-001", RunMode::Single, "local").unwrap();
    assert_eq!(run.steps.len(), 1);
    assert_eq!(run.steps[0].position, 1);
    assert_eq!(run.steps[0].step_id, None);
    assert_eq!(
        run.steps[0].driver_test.as_deref(),
        Some("test/system/booking_test.rb#BookingTest#test_happy_path")
    );

    repo::record_step(
        &db.conn,
        &run.id,
        1,
        RunStatus::Passed,
        "ok",
        "",
        Some(12),
        run.steps[0].driver_test.as_deref(),
        "1 runs, 0 failures",
    )
    .unwrap();
    let finished = repo::finish_run(&db.conn, &run.id).unwrap();
    assert_eq!(finished.status, RunStatus::Passed);

    // Composed keeps one row per case step.
    let composed = repo::create_run(&db.conn, "TC-001", RunMode::Composed, "local").unwrap();
    assert_eq!(composed.steps.len(), 2);
    assert_eq!(composed.steps[1].position, 2);
}

#[test]
fn attachments_support_case_roles_and_the_owner_check() {
    let db = Db::open_memory().unwrap();
    case(&db, "TC-001", "Happy path", "high", &["smoke"]);
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("happy.png");
    std::fs::write(&source, b"png").unwrap();

    let attachment = repo::attach_file_with_role(
        &db.conn,
        dir.path(),
        repo::EvidenceOwner::Case("TC-001".to_string()),
        &source,
        "expected happy path",
        "expected",
    )
    .unwrap();
    assert_eq!(attachment.case_id.as_deref(), Some("TC-001"));
    assert_eq!(attachment.role, "expected");
    assert_eq!(attachment.step_run_id, None);

    let evidence = repo::case_evidence(&db.conn, "TC-001").unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].role, "expected");

    let default_role = repo::attach_file(
        &db.conn,
        dir.path(),
        repo::EvidenceOwner::Case("TC-001".to_string()),
        &source,
        "another",
    )
    .unwrap();
    assert_eq!(default_role.role, "evidence");

    let bad = repo::attach_file_with_role(
        &db.conn,
        dir.path(),
        repo::EvidenceOwner::Case("TC-001".to_string()),
        &source,
        "",
        "not-a-role",
    );
    assert!(bad.is_err());

    let orphan = db.conn.execute(
        "INSERT INTO attachment (path,created_at) VALUES ('x','now')",
        [],
    );
    assert!(
        orphan.is_err(),
        "owner check must reject orphan attachments"
    );
}

#[test]
fn entity_links_are_idempotent_bidirectional_and_deletable() {
    let db = Db::open_memory().unwrap();
    case(&db, "TC-001", "Happy path", "high", &["smoke"]);
    let bug = repo::create_bug(
        &db.conn,
        Some("BUG-1"),
        "Broken",
        "",
        "1. click",
        "500",
        "200",
        1,
        2,
        "local",
        Some("TC-001"),
        None,
        None,
        "",
    )
    .unwrap();

    let first = repo::create_link(
        &db.conn,
        "bug",
        &bug.id,
        "case",
        "TC-001",
        "reproduces",
        "found on the booking page",
    )
    .unwrap();
    let again = repo::create_link(
        &db.conn,
        "bug",
        &bug.id,
        "case",
        "TC-001",
        "reproduces",
        "duplicate",
    )
    .unwrap();
    assert_eq!(first.id, again.id);

    let from_case = repo::list_links(&db.conn, "case", "TC-001").unwrap();
    assert_eq!(from_case.len(), 1);
    assert_eq!(from_case[0].relation, "reproduces");
    let from_bug = repo::list_links(&db.conn, "bug", &bug.id).unwrap();
    assert_eq!(from_bug.len(), 1);

    assert!(repo::delete_link(&db.conn, first.id).unwrap());
    assert!(!repo::delete_link(&db.conn, first.id).unwrap());
    assert!(
        repo::list_links(&db.conn, "case", "TC-001")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn email_logs_and_recent_recipients_are_recorded() {
    let db = Db::open_memory().unwrap();
    repo::create_email_log(
        &db.conn,
        "ana@example.com",
        "Report",
        "/tmp/report.pdf",
        "written",
        "eml",
        "/tmp/out.eml",
        "",
    )
    .unwrap();
    repo::create_email_log(
        &db.conn,
        "bo@example.com",
        "Second",
        "",
        "failed",
        "smtp",
        "",
        "no credentials",
    )
    .unwrap();

    let logs = repo::list_email_logs(&db.conn, 10).unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].to_addr, "bo@example.com");
    assert_eq!(logs[0].status, "failed");
    assert_eq!(logs[1].to_addr, "ana@example.com");

    let recipients = repo::recent_email_recipients(&db.conn, 5).unwrap();
    assert_eq!(
        recipients,
        vec!["bo@example.com".to_string(), "ana@example.com".to_string()]
    );
}
