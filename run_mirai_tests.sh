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

# Run the checker fixture target explicitly, then invoke the double-lock positive control and
# capture-reconstruction known limitation directly so both expected outcomes are visible.
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

positive_fixture="checker/tests/run-pass/model_field_wrapper_field_double_lock.rs"
positive_output="$(
    target/debug/mirai \
        --crate-name mirai \
        "$positive_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
printf '%s\n' "$positive_output"

if ! grep -q 'warning: \[MIRAI\] unsatisfied precondition' <<<"$positive_output"; then
    echo "The model-field double-lock positive control did not fire." >&2
    exit 1
fi

known_limit_fixture="checker/tests/run-pass/arc_load_thin_pointer_known_limitation.rs"
known_limit_output="$(
    target/debug/mirai \
        --crate-name mirai \
        "$known_limit_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
printf '%s\n' "$known_limit_output"

if grep -q 'warning: \[MIRAI\] unsatisfied precondition' <<<"$known_limit_output"; then
    echo "The callback capture-reconstruction known limitation unexpectedly fired." >&2
    exit 1
fi

echo "MIRAI test suite passed; positive control fired and capture-reconstruction XFAIL stayed silent."
