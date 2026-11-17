# Minitest fixtures

These fixtures exercise the Minitest driver without a Rails app, database, or
browser.

## Files

| File | Purpose |
|---|---|
| `test/models/user_test.rb` | Fake Rails Minitest file with two tests: one `test "..." do` block and one `def test_...`. |
| `output_pass.txt` | Captured verbose output for a green run, with ANSI color codes. |
| `output_fail.txt` | Captured verbose output for a run with one failure, with ANSI color codes. |

## Discovery expectations

Discovery walks `test/**/*_test.rb` and produces:

```text
test/models/user_test.rb#UserTest#test_valid_user            (line 4)
test/models/user_test.rb#UserTest#test_rejects_blank_name    (line 8)
```

`test "valid user" do` becomes `test_valid_user` (Minitest replaces whitespace
with underscores). Metadata also carries `class`, `method`, `kind`, and
`description`.

## Parsing expectations

`output_pass.txt`:

- `UserTest#test_valid_user` — passed, 12 ms
- `UserTest#test_rejects_blank_name` — passed, 4 ms
- summary: `2 runs, 2 assertions, 0 failures, 0 errors, 0 skips`

`output_fail.txt`:

- `UserTest#test_valid_user` — passed
- `UserTest#test_rejects_blank_name` — failed, message
  `Expected true to be false.`
- summary: `2 runs, 2 assertions, 1 failures, 0 errors, 0 skips`

ANSI escape codes are present in both files so tests prove they are stripped
from `TestOutcome::raw_output`.

## Notes

- The Ruby file is never executed by these fixtures. `User` is intentionally
  undefined; only the parser and discovery are under test.
- `src/driver/minitest.rs` locates this directory with
  `CARGO_MANIFEST_DIR`, so tests work from any working directory.
- The golden JSON projection for reports is owned by the report pod and is not
  part of this fixture set yet.
