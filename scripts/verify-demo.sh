#!/bin/sh
# Verify the seeded Laburo v0.2 demo without changing it.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROJECT=${1:-"$ROOT/../src-elizabeth-tours"}
LABURO=${LABURO:-laburo}
FAIL=0

ok() { printf '[ok]   %s\n' "$1"; }
bad() { printf '[FAIL] %s\n' "$1"; FAIL=1; }

check() {
    label=$1
    shift
    if "$@" >/dev/null 2>&1; then ok "$label"; else bad "$label"; fi
}

printf 'Laburo demo verification\n'
printf 'project: %s\n\n' "$PROJECT"

check "binary runs" "$LABURO" --version
check "database exists" test -f "$PROJECT/.laburo/laburo.sqlite3"
check "Rails app" test -f "$PROJECT/bin/rails"
check "TQA dictionary" test -f "$PROJECT/tqa/casos.yaml"
check "expected screenshot" test -f "$PROJECT/tmp/laburo-evidence/happy-path.png"

cases=$("$LABURO" --project "$PROJECT" case list 2>/dev/null | wc -l)
[ "$cases" -ge 21 ] && ok "catalogue ($cases cases)" || bad "catalogue ($cases cases, expected >=21)"

runs=$("$LABURO" --project "$PROJECT" run list 2>/dev/null | wc -l)
[ "$runs" -ge 3 ] && ok "runs ($runs)" || bad "runs ($runs, expected >=3)"

bugs=$("$LABURO" --project "$PROJECT" bug list 2>/dev/null | wc -l)
[ "$bugs" -ge 1 ] && ok "bugs ($bugs)" || bad "bugs ($bugs, expected >=1)"

pdfs=$(find "$PROJECT/.laburo/reports" -name report.pdf 2>/dev/null | wc -l)
[ "$pdfs" -ge 4 ] && ok "class PDFs ($pdfs)" || bad "class PDFs ($pdfs, expected >=4)"

emls=$(find "$PROJECT/.laburo/outbox" -name '*.eml' 2>/dev/null | wc -l)
[ "$emls" -ge 1 ] && ok "email outbox ($emls .eml)" || bad "email outbox ($emls, expected >=1)"

if printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"laburo_check_in","arguments":{"tag":"smoke"}}}' |
    "$LABURO" --project "$PROJECT" mcp 2>/dev/null | grep -q 'failing_runs'; then
    ok "MCP laburo_check_in"
else
    bad "MCP laburo_check_in"
fi

if curl -sf -o /dev/null http://127.0.0.1:3000/; then
    ok "web UI at http://127.0.0.1:3000"
else
    printf '[warn] web UI not running; start with: %s --project %s web --port 3000\n' "$LABURO" "$PROJECT"
fi

printf '\n'
if [ "$FAIL" -ne 0 ]; then
    printf 'Demo verification FAILED.\n'
    exit 1
fi
printf 'Demo verified. Open http://127.0.0.1:3000 and follow DEMO.md.\n'
