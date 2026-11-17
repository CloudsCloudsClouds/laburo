# MCP server

Status: design contract; minimal server in this wave
Last updated: 2026-11-17

Laburo exposes a **stdio MCP server** so an agent can operate the tool end to
end: discover tests, create cases, run them, record evidence, file bugs, and
generate reports. It uses the same core library as the TUI/CLI; no shelling out
to the CLI.

## Command

```bash
laburo mcp          # stdio JSON-RPC, for OpenCode/Claude/other MCP clients
```

The server speaks JSON-RPC 2.0 over stdin/stdout, one message per line
(Content-Length framing is also accepted if the client sends it). It writes
diagnostics to stderr only.

## Handshake

- `initialize` -> protocol version, server info, capabilities (`tools`).
- `notifications/initialized` -> no-op.
- `tools/list` -> tool definitions with JSON Schema.
- `tools/call` -> `{name, arguments}` -> result content.
- `ping` -> pong.
- Unknown method -> JSON-RPC error `-32601`.

## Tools

| Tool | Arguments | Result |
|---|---|---|
| `laburo_doctor` | none | environment/project checks |
| `laburo_list_cases` | `tag?`, `status?` | case summaries |
| `laburo_get_case` | `id` | full case + steps + tags |
| `laburo_create_case` | `id`, `title`, `description?`, `tags?`, `steps?` | created case |
| `laburo_update_case` | `id`, fields | updated case |
| `laburo_delete_case` | `id` | confirmation |
| `laburo_discover_tests` | `driver?` | discovered tests |
| `laburo_run_case` | `id`, `mode?`, `keep_going?` | run id + status + step results |
| `laburo_list_runs` | `case?`, `limit?` | run summaries |
| `laburo_get_run` | `id` | run + steps + evidence |
| `laburo_create_bug` | `case_id?`, `run_id?`, fields | bug id |
| `laburo_list_bugs` | `status?`, `severity?` | bug summaries |
| `laburo_attach_evidence` | `owner` (`step_run`/`run`/`bug`), `id`, `path`, `caption?` | attachment id |
| `laburo_generate_report` | `kind` (`dictionary`/`case`/`run`/`bug`), `id?` | PDF path |
| `laburo_quality_check` | `severity?` | findings + next action |
| `laburo_dashboard` | none | counts, pass rate, recent failures |

Manual runs via MCP: `laburo_run_case` creates the run and returns step IDs; a
second tool `laburo_record_step` records pass/fail/blocked + notes/evidence.
Automated runs execute the driver and return when done.

## Resources (optional)

- `laburo://cases` — case summaries.
- `laburo://runs` — recent runs.
- `laburo://bugs` — open bugs.
- `laburo://quality` — latest findings.

## Security

- Local only; no network listener.
- Validate IDs and paths; never pass user text to a shell.
- Evidence paths must resolve under the project or an allowed temp dir.
- No secrets in tool output.
- Destructive tools (`delete_case`, `restore`) require an explicit `confirm`
  flag, even over MCP.

## Implementation

- `src/mcp.rs`: transport, dispatch, JSON-RPC types.
- Tools call `src/repo/`, `src/driver/`, `src/report/`, `src/ai/`.
- A smoke test spawns the binary, performs `initialize`, `tools/list`, and one
  `tools/call` against a temp DB.
- No MCP SDK dependency required; serde_json is enough for v1.
