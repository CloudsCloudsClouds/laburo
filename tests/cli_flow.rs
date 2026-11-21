use std::process::Command;

fn laburo(project: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_laburo"))
        .arg("--project")
        .arg(project)
        .args(args)
        .output()
        .expect("run laburo")
}

#[test]
fn cli_end_to_end_without_drivers_or_reports() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();

    let init = laburo(project, &["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let create = laburo(
        project,
        &[
            "case",
            "create",
            "--id",
            "TC-001",
            "--title",
            "Booking happy path",
            "--description",
            "Tourist books a tour",
            "--module",
            "M-02",
            "--priority",
            "high",
            "--tag",
            "integration",
            "--tag",
            "functional",
            "--step",
            "Open catalogue|The catalogue is visible",
            "--step",
            "Submit booking|The booking is created",
        ],
    );
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    let stdout = String::from_utf8_lossy(&create.stdout);
    assert!(stdout.contains("TC-001"), "{stdout}");

    let list = laburo(project, &["case", "list"]);
    assert!(list.status.success());
    assert!(String::from_utf8_lossy(&list.stdout).contains("Booking happy path"));

    let show = laburo(project, &["case", "show", "TC-001"]);
    assert!(show.status.success());
    let show_out = String::from_utf8_lossy(&show.stdout);
    assert!(show_out.contains("Open catalogue"), "{show_out}");

    let evidence = project.join("evidence.log");
    std::fs::write(&evidence, b"step 1 ok\nstep 2 500\n").unwrap();

    let run = laburo(
        project,
        &[
            "run",
            "manual",
            "TC-001",
            "--step-status",
            "passed,failed",
            "--note",
            "second step failed",
            "--evidence",
            evidence.to_str().unwrap(),
        ],
    );
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let run_out = String::from_utf8_lossy(&run.stdout);
    assert!(run_out.contains("failed"), "{run_out}");
    let run_id = run_out.split_whitespace().next().unwrap().to_string();
    assert!(run_id.starts_with("RUN-"), "{run_id}");

    let bug = laburo(
        project,
        &[
            "bug",
            "create",
            "--title",
            "Booking returns 500",
            "--case",
            "TC-001",
            "--run",
            &run_id,
            "--severity",
            "1",
            "--actual",
            "500",
            "--expected",
            "booking created",
        ],
    );
    assert!(
        bug.status.success(),
        "{}",
        String::from_utf8_lossy(&bug.stderr)
    );
    assert!(String::from_utf8_lossy(&bug.stdout).contains("BUG-"));

    let bugs = laburo(project, &["bug", "list"]);
    assert!(bugs.status.success());
    assert!(String::from_utf8_lossy(&bugs.stdout).contains("Booking returns 500"));

    let quality = laburo(project, &["quality", "--json"]);
    assert!(quality.status.success());

    let doctor = laburo(project, &["doctor"]);
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stderr)
    );
}

#[test]
fn cli_batch_automation_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path();
    assert!(laburo(project, &["init"]).status.success(), "init failed");
    let create = laburo(
        project,
        &[
            "case",
            "create",
            "--id",
            "TC-BATCH",
            "--title",
            "Batch happy path",
            "--tag",
            "smoke",
            "--tag",
            "functional",
            "--step",
            "Open the catalogue|The catalogue is visible",
        ],
    );
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );

    let automation = laburo(
        project,
        &[
            "case",
            "set-automation",
            "TC-BATCH",
            "--mode",
            "single",
            "--target",
            "test/models/user_test.rb#UserTest#test_valid",
        ],
    );
    assert!(
        automation.status.success(),
        "{}",
        String::from_utf8_lossy(&automation.stderr)
    );
    let automation_out = String::from_utf8_lossy(&automation.stdout);
    assert!(automation_out.contains("mode=single"), "{automation_out}");
    assert!(
        automation_out.contains("test/models/user_test.rb#UserTest#test_valid"),
        "{automation_out}"
    );

    // A manual case stays manual-required and is reported, not silently ignored.
    assert!(
        laburo(
            project,
            &[
                "case",
                "create",
                "--id",
                "TC-MANUAL",
                "--title",
                "Manual checks",
                "--tag",
                "smoke",
                "--tag",
                "functional",
            ],
        )
        .status
        .success()
    );

    let batch = laburo(
        project,
        &[
            "run",
            "batch",
            "--tag",
            "smoke",
            "--type",
            "functional",
            "--status",
            "active",
            "--mode",
            "auto",
            "--dry-run",
        ],
    );
    assert!(
        batch.status.success(),
        "{}",
        String::from_utf8_lossy(&batch.stderr)
    );
    let batch_out = String::from_utf8_lossy(&batch.stdout);
    assert!(batch_out.contains("CREATED"), "{batch_out}");
    assert!(
        batch_out.contains("TC-MANUAL") && batch_out.contains("manual-required"),
        "{batch_out}"
    );
    assert!(
        batch_out.contains("attempted=2") && batch_out.contains("created=1"),
        "{batch_out}"
    );
    assert!(batch_out.contains("manual-required=1"), "{batch_out}");

    let add = laburo(
        project,
        &[
            "link",
            "add",
            "bug",
            "BUG-1",
            "case",
            "TC-BATCH",
            "--relation",
            "reproduces",
            "--note",
            "demo link",
        ],
    );
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add_out = String::from_utf8_lossy(&add.stdout);
    let link_id = add_out
        .split_whitespace()
        .next()
        .expect("link id")
        .trim_end_matches(':')
        .to_string();

    let list = laburo(project, &["link", "list", "case", "TC-BATCH"]);
    assert!(list.status.success());
    let list_out = String::from_utf8_lossy(&list.stdout);
    assert!(list_out.contains("BUG-1"), "{list_out}");
    assert!(list_out.contains("reproduces"), "{list_out}");

    let remove = laburo(project, &["link", "remove", &link_id]);
    assert!(
        remove.status.success(),
        "{}",
        String::from_utf8_lossy(&remove.stderr)
    );
    let after = laburo(project, &["link", "list", "case", "TC-BATCH"]);
    assert!(String::from_utf8_lossy(&after.stdout).contains("no links"));

    let screenshot = project.join("happy.png");
    std::fs::write(&screenshot, b"not really a png").unwrap();
    let evidence = laburo(
        project,
        &[
            "case",
            "evidence",
            "TC-BATCH",
            "--path",
            screenshot.to_str().unwrap(),
            "--caption",
            "happy path screenshot",
            "--role",
            "expected",
        ],
    );
    assert!(
        evidence.status.success(),
        "{}",
        String::from_utf8_lossy(&evidence.stderr)
    );
    let evidence_out = String::from_utf8_lossy(&evidence.stdout);
    assert!(evidence_out.contains("role=expected"), "{evidence_out}");

    let db = laburo::db::Db::open(&project.join(".laburo").join("laburo.sqlite3")).unwrap();
    let attachments = laburo::repo::case_evidence(&db.conn, "TC-BATCH").unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].role, "expected");
    assert_eq!(attachments[0].case_id.as_deref(), Some("TC-BATCH"));
    assert_eq!(attachments[0].caption, "happy path screenshot");
}
