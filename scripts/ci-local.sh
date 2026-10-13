#!/usr/bin/env bash
# One reproducible gate; launch through the shared host's guarded runner.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo +1.90 fmt --check
cargo +1.90 test --locked
cargo +1.90 clippy --locked --all-targets -- -D warnings
if [[ -n ${WAFER_TEST_SIOXC:-} && -n ${WAFER_TEST_STD:-} ]]; then
    "$WAFER_TEST_SIOXC" --help | grep -q -- '--module'
    "$WAFER_TEST_SIOXC" --help | grep -q -- '--test-source'
    cargo +1.90 test --locked --test compiler_execution -- --ignored --nocapture --test-threads=1
elif [[ ${WAFER_REQUIRE_NATIVE:-0} == 1 ]]; then
    echo 'native gate requires WAFER_TEST_SIOXC and WAFER_TEST_STD' >&2
    exit 2
else
    echo 'native gate not run: set WAFER_TEST_SIOXC/WAFER_TEST_STD to a compatible actual compiler' >&2
fi
