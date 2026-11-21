-- Laburo schema v3
-- Evidence links, case-level automation, and the email outbox log.
-- Applied by PRAGMA user_version. Keep migrations append-only.

PRAGMA foreign_keys = ON;

-- Case-level automation: a target for `single` mode and the default mode a
-- case uses when a batch does not request an explicit mode.
ALTER TABLE test_case ADD COLUMN driver_target TEXT;
ALTER TABLE test_case ADD COLUMN default_mode TEXT NOT NULL DEFAULT 'manual'
    CHECK (default_mode IN ('manual', 'single', 'composed'));

-- Rebuild attachment so evidence can also belong to a case (expected
-- artifacts), and record the role the attachment plays.
CREATE TABLE attachment_v3 (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    kind          TEXT NOT NULL DEFAULT 'file',
    path          TEXT NOT NULL,
    original_name TEXT NOT NULL DEFAULT '',
    mime          TEXT NOT NULL DEFAULT '',
    sha256        TEXT NOT NULL DEFAULT '',
    size          INTEGER NOT NULL DEFAULT 0,
    caption       TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL,
    step_run_id   INTEGER REFERENCES step_run(id) ON DELETE CASCADE,
    run_id        TEXT REFERENCES test_run(id) ON DELETE CASCADE,
    bug_id        TEXT REFERENCES bug_report(id) ON DELETE CASCADE,
    case_id       TEXT REFERENCES test_case(id) ON DELETE CASCADE,
    role          TEXT NOT NULL DEFAULT 'evidence',
    CHECK (
        step_run_id IS NOT NULL
        OR run_id IS NOT NULL
        OR bug_id IS NOT NULL
        OR case_id IS NOT NULL
    )
);

INSERT INTO attachment_v3 (
    id, kind, path, original_name, mime, sha256, size, caption, created_at,
    step_run_id, run_id, bug_id, case_id, role
)
SELECT
    id, kind, path, original_name, mime, sha256, size, caption, created_at,
    step_run_id, run_id, bug_id, NULL, 'evidence'
FROM attachment;

DROP TABLE attachment;
ALTER TABLE attachment_v3 RENAME TO attachment;

CREATE INDEX IF NOT EXISTS idx_attachment_step ON attachment(step_run_id);
CREATE INDEX IF NOT EXISTS idx_attachment_run ON attachment(run_id);
CREATE INDEX IF NOT EXISTS idx_attachment_bug ON attachment(bug_id);
CREATE INDEX IF NOT EXISTS idx_attachment_case ON attachment(case_id);

-- Generic graph edge between any two entities (case, run, bug, requirement,
-- step, ...). `relation` is one of reproduces, found-in, covers, related,
-- evidence-for; callers may add project-specific values.
CREATE TABLE IF NOT EXISTS entity_link (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_kind TEXT NOT NULL,
    source_id   TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    relation    TEXT NOT NULL,
    note        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    UNIQUE (source_kind, source_id, target_kind, target_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_entity_link_source
    ON entity_link(source_kind, source_id);
CREATE INDEX IF NOT EXISTS idx_entity_link_target
    ON entity_link(target_kind, target_id);

-- One row per outbox write or transport attempt.
CREATE TABLE IF NOT EXISTS email_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at  TEXT NOT NULL,
    to_addr     TEXT NOT NULL,
    subject     TEXT NOT NULL,
    report_path TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL,
    transport   TEXT NOT NULL,
    file_path   TEXT NOT NULL DEFAULT '',
    error       TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_email_log_created ON email_log(created_at);

-- Filter/index support for tag categories and case status/priority.
CREATE INDEX IF NOT EXISTS idx_tag_category ON tag(category);
CREATE INDEX IF NOT EXISTS idx_case_status_priority ON test_case(status, priority);
