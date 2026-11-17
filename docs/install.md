# Install and runtime

Status: frozen for the wave
Last updated: 2026-11-17

## Install

```bash
cd laburo
cargo install --path . --locked
laburo doctor
```

`cargo install` is the simplest reliable path on this machine. It puts `laburo`
in `~/.cargo/bin`, which is already on PATH.

For development:

```bash
cargo run -- doctor
cargo run -- --help
cargo run -- tui
```

## Project resolution

`--project <dir>` wins. Otherwise Laburo searches upward from the current
directory for:

1. `.laburo/`
2. `Gemfile` + `bin/rails` (Rails app)
3. `Cargo.toml`

The resolved root is stored in the config/DB metadata.

## Runtime data

```text
<project>/.laburo/
├── laburo.sqlite3
├── evidence/
├── reports/
└── tmp/
```

- `.laburo/` is added to `.gitignore` by the installer/init command; it is local
  data, not source.
- The SQLite DB uses WAL, foreign keys, and `synchronous=NORMAL`.
- Evidence files are copied and hashed; the DB stores relative paths.
- Reports keep PDF + `.typ` + `data.json`.

## Global config

`~/.config/laburo/config.toml` (or `$XDG_CONFIG_HOME/laburo/config.toml`):

```toml
[project]
root = "/home/danielb/MyProjects/PII-eliz-tours/src-elizabeth-tours"
default_driver = "minitest"

[reports]
typst = "typst"
output = ".laburo/reports"

[ai]
enabled = false
base_url = "http://localhost:11434/v1"
model = "qwen2.5:1.5b"
api_key_env = "LABURO_AI_KEY"
timeout_ms = 20000
```

Environment overrides: `LABURO_PROJECT`, `LABURO_DB`, `LABURO_AI_KEY`,
`LABURO_TYPST`.

## Doctor

`laburo doctor` checks:

- project root and Rails app (`bin/rails`);
- SQLite open/migrate;
- `typst` binary and version;
- `minitest` presence in the Gemfile;
- optional `cargo nextest`;
- optional AI endpoint (no network unless `--check-ai`).

## Backup / restore

```bash
laburo db backup                 # VACUUM INTO .laburo/backups/<timestamp>.sqlite3
laburo db restore <file>         # confirm, then replace
```

## Uninstall

```bash
cargo uninstall laburo
rm -rf <project>/.laburo   # destructive; confirm
```

## CI

GitHub Actions:

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- Typst template compile against a golden JSON projection.
- MCP stdio smoke test.

The thesis root is untouched; Laburo is its own repo.
