#!/usr/bin/env bash
set -euo pipefail

# Runs MIRAI's standard Cargo test suite and its run-pass checker fixtures.
# The repository's rust-toolchain.toml pins nightly-2026-06-01 and requests
# rustc-dev, rust-src, rustfmt, clippy, rust-std, and llvm-tools-preview.
# Building the default bundled Z3 also requires the normal C/C++ build toolchain.

repository_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repository_root"

cargo build --tests
cargo test

# Run the checker fixture target explicitly, then invoke its double-lock positive
# control directly so the expected MIRAI diagnostic is visible in this script's output.
cargo test -p mirai --test integration_tests run_pass -- --exact
cargo build -p mirai --bin mirai

sysroot="$(rustc --print sysroot)"
annotations="$(find target/debug/deps -maxdepth 1 -type f \
    -name 'libmirai_annotations-*.rlib' -print -quit)"
if [[ -z "$annotations" ]]; then
    echo "Could not locate the built mirai_annotations rlib." >&2
    exit 1
fi

output_dir="$(mktemp -d)"
trap 'rm -rf -- "$output_dir"' EXIT
export LD_LIBRARY_PATH="$sysroot/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

fixture="checker/tests/run-pass/model_field_wrapper_field_double_lock.rs"
fixture_output="$(
    target/debug/mirai \
        --crate-name mirai \
        "$fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
printf '%s\n' "$fixture_output"

if ! grep -q 'warning: \[MIRAI\] unsatisfied precondition' <<<"$fixture_output"; then
    echo "The model-field double-lock positive control did not fire." >&2
    exit 1
fi

echo "MIRAI test suite passed; model-field double-lock positive control fired."
