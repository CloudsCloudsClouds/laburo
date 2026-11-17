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
