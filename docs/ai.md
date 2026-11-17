# AI quality engine (Tier 2)

Status: design contract; implementation is Tier 2
Last updated: 2026-11-17

The AI is a **quality safeguard**, not an author. Deterministic rules generate
candidate findings and actions; a local model ranks/chooses and explains; the
human accepts. The model never writes to SQLite directly.

## Why this shape

- Tiny local models are good at ranking/classifying, not at being a source of
  truth.
- A rule engine gives deterministic, explainable candidates with IDs.
- The model adds judgment about priority and next action.
- Everything is traceable: each finding cites the case/run/bug/step it came from.
- Tier 1 works with no model at all.

## Provider

OpenAI-compatible API, configured in `~/.config/laburo/config.toml` or env:

```toml
[ai]
enabled = true
base_url = "http://localhost:11434/v1"   # Ollama
model = "qwen2.5:1.5b"                   # any local/remote model
api_key_env = "LABURO_AI_KEY"            # optional for local
timeout_ms = 20000
```

- Ollama works through its `/v1` compatibility endpoint.
- Remote OpenAI-compatible providers use the same code path.
- No network calls when `ai.enabled = false`.
- The model call is in a worker thread and never blocks the TUI.

## Rule candidates

Examples (all deterministic):

- Case has no steps.
- Step has no expected result.
- Failed step has no bug report.
- Bug is open without reproduction steps.
- Case has no tags, or no type/level tag.
- Case has no evidence on a failed run.
- Same case passes and fails across runs (flaky).
- Case not run in N days.
- Requirement has no test case.
- Duplicate case titles.
- Run failed but the report was generated as if green.
- Open critical bug older than N days.

Each candidate has: `id`, `kind`, `severity`, `reason`, `record_refs`,
`suggested_action`, and `deterministic_context`.

## Model tasks

1. **Rank** candidates by risk/impact.
2. **Choose the next action** from the candidate list.
3. **Explain** the choice using the cited records.
4. **Draft** a bug report, Gherkin scenario, or test case from provided records.
5. **Summarize** a run/report using only provided numbers.

The prompt includes only the records and the candidate list. The response is
parsed as JSON and validated. On timeout/invalid output, fall back to the
deterministic ranking and show a warning.

## Tier 2 features

| Feature | Input | Output |
|---|---|---|
| Quality review | project state | ranked findings + next action |
| Failure triage | failed run + logs | clustered causes + proposed bug |
| Gherkin generation | case/use case | Feature/Scenario text for review |
| Test case drafting | requirement | proposed case(s), status `draft` |
| Run summary | run/report data | narrative grounded in numbers |
| Project memory | FTS5 index | cited answers over cases/runs/bugs/notes |
| Test maintenance | run history | flaky/stale findings + proposed updates |
| Metrics insight | deterministic dashboard | explanation, no new numbers |

## Safeguards

- No direct writes; suggestions require an explicit accept action.
- Every claim must cite a record ID; reject answers without citations.
- Numbers come from SQLite, never from the model.
- Secrets and evidence paths are redacted from prompts.
- Timeouts and a hard token budget.
- `laburo quality check` works without AI and returns non-zero on high findings.
