-- Laburo schema v1
-- Applied by PRAGMA user_version. Keep migrations append-only.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS test_case (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    module      TEXT NOT NULL DEFAULT '',
    priority    TEXT NOT NULL DEFAULT 'medium',
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('draft', 'active', 'archived')),
    environment TEXT NOT NULL DEFAULT 'local',
    gherkin     TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS test_step (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    case_id       TEXT NOT NULL REFERENCES test_case(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    action        TEXT NOT NULL,
    data          TEXT NOT NULL DEFAULT '',
    expected      TEXT NOT NULL DEFAULT '',
    kind          TEXT NOT NULL DEFAULT 'manual'
                  CHECK (kind IN ('manual', 'driver')),
    driver_target TEXT,
    UNIQUE (case_id, position)
);

CREATE TABLE IF NOT EXISTS tag (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    name     TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL DEFAULT 'custom'
);

CREATE TABLE IF NOT EXISTS case_tag (
    case_id TEXT NOT NULL REFERENCES test_case(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    PRIMARY KEY (case_id, tag_id)
);

CREATE TABLE IF NOT EXISTS test_run (
    id          TEXT PRIMARY KEY,
    case_id     TEXT NOT NULL REFERENCES test_case(id) ON DELETE CASCADE,
    mode        TEXT NOT NULL DEFAULT 'manual'
                CHECK (mode IN ('manual', 'single', 'composed')),
    status      TEXT NOT NULL DEFAULT 'not_run'
                CHECK (status IN ('not_run', 'running', 'passed', 'failed', 'blocked', 'error')),
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    environment TEXT NOT NULL DEFAULT 'local',
    commit_ref  TEXT NOT NULL DEFAULT '',
    command     TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS step_run (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    step_id     INTEGER REFERENCES test_step(id) ON DELETE SET NULL,
    position    INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'not_run'
                CHECK (status IN ('not_run', 'running', 'passed', 'failed', 'blocked', 'error')),
    observed    TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER,
    driver_test TEXT,
    raw_output  TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS bug_report (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    steps       TEXT NOT NULL DEFAULT '',
    actual      TEXT NOT NULL DEFAULT '',
    expected    TEXT NOT NULL DEFAULT '',
    priority    INTEGER NOT NULL DEFAULT 3,
    severity    INTEGER NOT NULL DEFAULT 3,
    environment TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'open'
                CHECK (status IN ('open', 'resolved', 'closed', 'reopened')),
    case_id     TEXT REFERENCES test_case(id) ON DELETE SET NULL,
    run_id      TEXT REFERENCES test_run(id) ON DELETE SET NULL,
    step_run_id INTEGER REFERENCES step_run(id) ON DELETE SET NULL,
    comments    TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS attachment (
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
    CHECK (step_run_id IS NOT NULL OR run_id IS NOT NULL OR bug_id IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS driver (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    name        TEXT NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    enabled     INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS driver_test (
    id            TEXT PRIMARY KEY,
    driver_id     TEXT NOT NULL REFERENCES driver(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    file          TEXT NOT NULL DEFAULT '',
    line          INTEGER,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    discovered_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS requirement (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    source     TEXT NOT NULL DEFAULT '',
    acceptance TEXT NOT NULL DEFAULT '',
    kind       TEXT NOT NULL DEFAULT 'functional'
);

CREATE TABLE IF NOT EXISTS case_requirement (
    case_id        TEXT NOT NULL REFERENCES test_case(id) ON DELETE CASCADE,
    requirement_id TEXT NOT NULL REFERENCES requirement(id) ON DELETE CASCADE,
    PRIMARY KEY (case_id, requirement_id)
);

CREATE INDEX IF NOT EXISTS idx_step_case ON test_step(case_id, position);
CREATE INDEX IF NOT EXISTS idx_run_case ON test_run(case_id, started_at);
CREATE INDEX IF NOT EXISTS idx_step_run_run ON step_run(run_id, position);
CREATE INDEX IF NOT EXISTS idx_attachment_step ON attachment(step_run_id);
CREATE INDEX IF NOT EXISTS idx_attachment_run ON attachment(run_id);
CREATE INDEX IF NOT EXISTS idx_attachment_bug ON attachment(bug_id);
CREATE INDEX IF NOT EXISTS idx_bug_case ON bug_report(case_id);
CREATE INDEX IF NOT EXISTS idx_bug_run ON bug_report(run_id);
CREATE INDEX IF NOT EXISTS idx_driver_test_driver ON driver_test(driver_id);

INSERT OR IGNORE INTO tag (name, category) VALUES
    ('unit', 'level'),
    ('integration', 'level'),
    ('system', 'level'),
    ('functional', 'type'),
    ('non-functional', 'type'),
    ('black-box', 'technique'),
    ('white-box', 'technique'),
    ('regression', 'quality'),
    ('smoke', 'quality'),
    ('sanity', 'quality'),
    ('acceptance', 'quality'),
    ('security', 'quality'),
    ('performance', 'quality'),
    ('load', 'quality'),
    ('stress', 'quality'),
    ('compatibility', 'quality'),
    ('installation', 'quality'),
    ('recovery', 'quality'),
    ('usability', 'quality'),
    ('localization', 'quality');
