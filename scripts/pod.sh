#!/bin/sh
# Per-pod cargo helper: separate target dirs avoid the cargo target lock when
# several agents build at once. sccache (when installed) shares compiled objects.
#
#   scripts/pod.sh tui check --all-targets
#   scripts/pod.sh reports test --test e2e_fixture
#
set -eu
if [ "$#" -lt 2 ]; then
    echo "usage: $0 <pod-name> <cargo args...>" >&2
    exit 2
fi
pod=$1
shift
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target/pods/$pod}"
if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="${RUSTC_WRAPPER:-sccache}"
fi
exec cargo "$@"
