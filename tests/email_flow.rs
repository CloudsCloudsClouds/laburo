use laburo::config::ProjectConfig;
use laburo::db::Db;
use laburo::email;
use std::path::{Path, PathBuf};

fn test_project() -> (tempfile::TempDir, ProjectConfig) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let data_dir = root.join(".laburo");
    let cfg = ProjectConfig {
        root,
        db_path: data_dir.join("laburo.sqlite3"),
        data_dir,
        typst: "typst".to_string(),
    };
    (dir, cfg)
}

fn fake_pdf(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(
        &path,
        b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n",
    )
    .unwrap();
    path
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        out.push(ALPHABET[(b[0] >> 2) as usize] as char);
        out.push(ALPHABET[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b[2] & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[test]
fn writes_eml_with_pdf_attachment_and_logs() {
    let (_dir, cfg) = test_project();
    let db = Db::open_memory().unwrap();

    let report = fake_pdf(&cfg.root, "run-report.pdf");
    let report_bytes = std::fs::read(&report).unwrap();

    let eml_path = email::write_eml(
        &cfg,
        &db.conn,
        "ana@example.com",
        "Booking report TC-001",
        "Adjunto el reporte de ejecucion.",
        &report,
    )
    .unwrap();

    assert!(eml_path.is_file(), "{}", eml_path.display());
    assert_eq!(eml_path.parent(), Some(email::outbox_dir(&cfg).as_path()));
    let name = eml_path.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.ends_with(".eml"), "{name}");
    assert!(
        name.contains("booking-report-tc-001"),
        "slug missing from {name}"
    );

    let eml = std::fs::read_to_string(&eml_path).unwrap();
    assert!(eml.contains("To:"), "{eml}");
    assert!(eml.contains("ana@example.com"), "{eml}");
    assert!(eml.contains("From:"), "{eml}");
    assert!(eml.contains("Laburo"), "{eml}");
    assert!(eml.contains("laburo@localhost"), "{eml}");
    assert!(eml.contains("Subject: Booking report TC-001"), "{eml}");
    assert!(
        eml.contains("Adjunto el reporte de ejecucion."),
        "text body missing"
    );
    assert!(eml.contains("Content-Type: application/pdf"), "{eml}");
    assert!(eml.contains("Content-Disposition: attachment"), "{eml}");
    assert!(eml.contains("run-report.pdf"), "{eml}");

    // MIME base64 is line-wrapped; compare against the eml with newlines removed.
    let flat: String = eml.chars().filter(|c| *c != '\r' && *c != '\n').collect();
    assert!(
        flat.contains(&base64(&report_bytes)),
        "base64 PDF payload missing"
    );

    let row = db
        .conn
        .query_row(
            "SELECT to_addr,subject,report_path,status,transport,file_path
             FROM email_log ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(row.0, "ana@example.com");
    assert_eq!(row.1, "Booking report TC-001");
    assert_eq!(row.2, report.to_string_lossy().to_string());
    assert_eq!(row.3, "written");
    assert_eq!(row.4, "eml");
    assert!(
        row.5.ends_with(&name),
        "file_path {} should end with {name}",
        row.5
    );
}

#[test]
fn rejects_missing_or_non_pdf_report() {
    let (_dir, cfg) = test_project();
    let db = Db::open_memory().unwrap();

    let missing = cfg.root.join("missing.pdf");
    let err = email::write_eml(&cfg, &db.conn, "ana@example.com", "S", "B", &missing)
        .unwrap_err()
        .to_string();
    assert!(err.contains("missing.pdf"), "{err}");

    let not_pdf = cfg.root.join("notes.pdf");
    std::fs::write(&not_pdf, b"this is not a pdf").unwrap();
    let err = email::write_eml(&cfg, &db.conn, "ana@example.com", "S", "B", &not_pdf)
        .unwrap_err()
        .to_string();
    assert!(err.contains("not a PDF"), "{err}");

    let count: i64 = db
        .conn
        .query_row("SELECT count(*) FROM email_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn recent_recipients_are_distinct_newest_first() {
    let db = Db::open_memory().unwrap();

    for (created_at, to) in [
        ("2026-01-01 10:00:00", "ana@example.com"),
        ("2026-01-01 11:00:00", "bruno@example.com"),
        ("2026-01-01 12:00:00", "ana@example.com"),
    ] {
        db.conn
            .execute(
                "INSERT INTO email_log (created_at,to_addr,subject,status,transport)
                 VALUES (?1,?2,'s','written','eml')",
                rusqlite::params![created_at, to],
            )
            .unwrap();
    }

    assert_eq!(
        email::recent_recipients(&db.conn).unwrap(),
        vec![
            "ana@example.com".to_string(),
            "bruno@example.com".to_string()
        ]
    );
}
