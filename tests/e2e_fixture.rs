use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_laburo")
}

fn run(project: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .arg("--project")
        .arg(project)
        .args(args)
        .output()
        .expect("run laburo")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn fixture_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project");
    let dst = dir.path().join("project");
    copy_dir(&src, &dst);
    (dir, dst)
}

#[test]
fn full_fixture_flow_with_driver_and_reports() {
    let (_guard, project) = fixture_project();
    let init = run(&project, &["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let evidence = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/evidence/log.txt");
    let create = run(
        &project,
        &[
            "case",
            "create",
            "--id",
            "TC-001",
            "--title",
            "Fixture booking",
            "--tag",
            "integration",
            "--step",
            "Open catalogue|The catalogue is visible",
            "--step",
            "Submit booking|The booking is created",
            "--step",
            "Confirm booking|The booking is confirmed",
        ],
    );
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );

    let manual = run(
        &project,
        &[
            "run",
            "manual",
            "TC-001",
            "--step-status",
            "passed,failed,passed",
            "--note",
            "step 2 broke",
            "--evidence",
            evidence.to_str().unwrap(),
        ],
    );
    assert!(
        manual.status.success(),
        "{}",
        String::from_utf8_lossy(&manual.stderr)
    );
    let manual_out = String::from_utf8_lossy(&manual.stdout);
    assert!(manual_out.contains("failed"), "{manual_out}");
    let run_id = manual_out.split_whitespace().next().unwrap().to_string();

    let show = run(&project, &["run", "show", &run_id]);
    assert!(show.status.success());
    let show_out = String::from_utf8_lossy(&show.stdout);
    assert!(show_out.contains("evidence"), "{show_out}");
    let step_id = show_out
        .lines()
        .find(|l| l.contains("[id ") && l.contains("failed"))
        .and_then(|l| l.split("[id ").nth(1))
        .and_then(|s| s.split(']').next())
        .expect("failed step id")
        .to_string();

    let bug = run(
        &project,
        &[
            "bug",
            "create",
            "--title",
            "Booking returns 500",
            "--case",
            "TC-001",
            "--run",
            &run_id,
            "--step",
            &step_id,
            "--severity",
            "1",
            "--actual",
            "500",
            "--expected",
            "booking created",
            "--evidence",
            evidence.to_str().unwrap(),
        ],
    );
    assert!(
        bug.status.success(),
        "{}",
        String::from_utf8_lossy(&bug.stderr)
    );
    let bug_out = String::from_utf8_lossy(&bug.stdout);
    let bug_id = bug_out.split_whitespace().next().unwrap().to_string();
    assert!(bug_id.starts_with("BUG-"), "{bug_out}");

    let discover = run(&project, &["driver", "discover", "--driver", "minitest"]);
    assert!(
        discover.status.success(),
        "{}",
        String::from_utf8_lossy(&discover.stderr)
    );
    let discover_out = String::from_utf8_lossy(&discover.stdout);
    assert!(
        discover_out.contains("discovered 2 tests"),
        "{discover_out}"
    );

    let auto = run(
        &project,
        &[
            "run",
            "auto",
            "TC-001",
            "--target",
            "test/models/user_test.rb#UserTest#test_valid",
        ],
    );
    assert!(
        auto.status.success(),
        "{}",
        String::from_utf8_lossy(&auto.stderr)
    );
    let auto_out = String::from_utf8_lossy(&auto.stdout);
    assert!(auto_out.contains("passed"), "{auto_out}");

    for args in [
        vec!["report", "dictionary"],
        vec!["report", "case", "TC-001"],
        vec!["report", "run", &run_id],
        vec!["report", "bug", &bug_id],
    ] {
        let out = run(&project, &args);
        assert!(
            out.status.success(),
            "report {:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        let pdf = String::from_utf8_lossy(&out.stdout).trim().to_string();
        assert!(Path::new(&pdf).is_file(), "missing PDF: {pdf}");
    }

    let quality = run(&project, &["quality", "--json"]);
    assert!(quality.status.success());
    let quality_out = String::from_utf8_lossy(&quality.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&quality_out).is_ok(),
        "{quality_out}"
    );
}

#[test]
fn mcp_initialize_list_and_dashboard() {
    let (_guard, project) = fixture_project();
    assert!(run(&project, &["init"]).status.success());
    assert!(
        run(
            &project,
            &["case", "create", "--id", "TC-001", "--title", "MCP case"]
        )
        .status
        .success()
    );

    let mut child = Command::new(bin())
        .arg("--project")
        .arg(&project)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
        )
        .unwrap();
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"laburo_dashboard","arguments":{{}}}}}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("laburo"), "{stdout}");
    assert!(stdout.contains("tools"), "{stdout}");
    assert!(stdout.contains("result"), "{stdout}");
}
