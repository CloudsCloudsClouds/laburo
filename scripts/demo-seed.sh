#!/bin/sh
# Laburo v0.2 demo seed. Resets only <project>/.laburo.
# Usage: scripts/demo-seed.sh [project] [--keep]
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROJECT=${1:-"$ROOT/../src-elizabeth-tours"}
KEEP=${2:-}
LABURO=${LABURO:-laburo}
EVIDENCE="$ROOT/tests/fixtures/evidence/log.txt"

if [ "$KEEP" != "--keep" ]; then
    rm -rf "$PROJECT/.laburo"
fi

"$LABURO" --project "$PROJECT" init
"$LABURO" --project "$PROJECT" import
"$LABURO" --project "$PROJECT" driver discover

tag() {
    "$LABURO" --project "$PROJECT" case tag "$1" --tag "$2" >/dev/null 2>&1 || true
}

tag TC-M02-01-1 smoke
tag TC-M02-01-1 sanity
tag TC-M02-01-1 integration
tag TC-M02-01-2 smoke
tag TC-M02-01-2 sanity
tag TC-M02-01-2 happy-path
tag TC-M02-01-2 integration
tag TC-M02-01-3 negative
tag TC-M02-01-4 negative
tag TC-M02-01-4 boundary
tag TC-M02-01-5 integration
tag TC-M02-01-5 addons
tag TC-M02-01-6 system
tag TC-M02-01-6 happy-path
tag TC-M03-01-1 unit
tag TC-M03-01-2 unit

find_test() {
    "$LABURO" --project "$PROJECT" driver list 2>/dev/null |
        awk -v pat="$1" '$0 ~ pat { print $1; exit }'
}

map_case() {
    case_id=$1
    pattern=$2
    target=$(find_test "$pattern")
    if [ -n "$target" ]; then
        "$LABURO" --project "$PROJECT" case set-automation "$case_id" \
            --mode single --target "$target" >/dev/null 2>&1 || true
        echo "mapped $case_id -> $target"
    fi
}

# Real Minitest targets from the Rails uplift.
map_case TC-M02-01-2 'smoke_happy_path_test.*test_public_catalogue'
map_case TC-M02-01-1 'customer_test.*test_builds_the_full_name'
map_case TC-M02-01-3 'booking_test.*test_rejects_a_booking_for_a_tour_that_does_not_exist'
map_case TC-M02-01-4 'booking_test.*test_travellers_1_and_20'
map_case TC-M02-01-5 'booking_test.*test_adds_every_selected_addon'
map_case TC-M03-01-1 'addon_test'
map_case TC-M02-01-6 'test/system/booking_happy_path_test'

# Expected artifact: the real happy-path screenshot from the Rails system test.
SHOT="$PROJECT/tmp/laburo-evidence/happy-path.png"
if [ -f "$SHOT" ]; then
    "$LABURO" --project "$PROJECT" case evidence TC-M02-01-2 \
        --path "$SHOT" --role expected --caption "Happy-path booking confirmation" \
        >/dev/null 2>&1 || true
    "$LABURO" --project "$PROJECT" case evidence TC-M02-01-1 \
        --path "$SHOT" --role expected --caption "Expected booking confirmation" \
        >/dev/null 2>&1 || true
fi

# A manual failed run with evidence, plus the bug linked to it.
"$LABURO" --project "$PROJECT" run manual TC-M02-01-1 \
    --step-status passed,failed,passed \
    --note "demo: step 2 fails" \
    --evidence "$EVIDENCE" >/dev/null
RUN=$("$LABURO" --project "$PROJECT" run list | head -1 | awk '{print $1}')
if [ -n "$RUN" ]; then
    BUG=$("$LABURO" --project "$PROJECT" bug create \
        --title "Booking step 2 returns 500" \
        --description "Demo bug from the failed happy-path step." \
        --case TC-M02-01-1 \
        --run "$RUN" \
        --severity 1 \
        --actual "500 error" \
        --expected "booking created" \
        --evidence "$EVIDENCE" 2>/dev/null | head -1 | awk '{print $1}')
    if [ -n "$BUG" ]; then
        "$LABURO" --project "$PROJECT" link add bug "$BUG" case TC-M02-01-1 \
            --relation reproduces --note "Standardized trigger" >/dev/null 2>&1 || true
        "$LABURO" --project "$PROJECT" link add bug "$BUG" run "$RUN" \
            --relation found-in >/dev/null 2>&1 || true
    fi
fi

# A real automated run for the happy-path case, if the driver target exists.
HAPPY=$(find_test 'smoke_happy_path_test.*test_public_catalogue')
if [ -n "$HAPPY" ]; then
    "$LABURO" --project "$PROJECT" run auto TC-M02-01-2 \
        --mode single --target "$HAPPY" >/dev/null 2>&1 || true
fi

SYSTEM=$(find_test 'test/system/booking_happy_path_test')
if [ -n "$SYSTEM" ]; then
    "$LABURO" --project "$PROJECT" run auto TC-M02-01-6 \
        --mode single --target "$SYSTEM" >/dev/null 2>&1 || true
fi

"$LABURO" --project "$PROJECT" report case TC-M02-01-2 >/dev/null 2>&1 || true
if [ -n "${RUN:-}" ]; then
    "$LABURO" --project "$PROJECT" report run "$RUN" >/dev/null 2>&1 || true
fi
if [ -n "${BUG:-}" ]; then
    "$LABURO" --project "$PROJECT" report bug "$BUG" >/dev/null 2>&1 || true
fi
"$LABURO" --project "$PROJECT" report dictionary >/dev/null
PDF=$(find "$PROJECT/.laburo/reports" -name report.pdf | sort | tail -1)
if [ -n "$PDF" ]; then
    "$LABURO" --project "$PROJECT" email send \
        --to demo@example.com --subject "Laburo demo report" --report "$PDF" \
        >/dev/null 2>&1 || true
fi

cat <<EOF
Laburo v0.2 demo seeded.
  project:  $PROJECT
  db:       $PROJECT/.laburo/laburo.sqlite3
  outbox:   $PROJECT/.laburo/outbox
  reports:  $PROJECT/.laburo/reports
Run:
  $LABURO web --project $PROJECT --port 3000
EOF
