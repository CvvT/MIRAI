#!/usr/bin/env bash
set -euo pipefail

# Runs MIRAI's standard Cargo test suite and its run-pass checker fixtures.
# The repository's rust-toolchain.toml pins nightly-2026-06-01 and requests
# rustc-dev, rust-src, rustfmt, clippy, rust-std, and llvm-tools-preview.
# Building the default bundled Z3 also requires the normal C/C++ build toolchain.

repository_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$repository_root"

# This runner builds MIRAI itself. Ignore analysis settings inherited from a
# prior MIRAI invocation and keep the artifact paths below rooted in this tree.
unset RUSTC_WORKSPACE_WRAPPER MIRAI_FLAGS MIRAI_LOG MIRAI_SHARE_PERSISTENT_STORE CARGO_TARGET_DIR
export RUST_SYSROOT="$(rustc --print sysroot)"

cargo build --tests
cargo test

# Run the checker fixture target explicitly, then invoke both synthetic double-lock positive
# controls directly so their expected outcomes are visible. The Arc-loaded capture fixture was a
# known limitation before the callback carrier hops landed; the shipped carrier lineage now
# derives its double-lock, so it is a positive control. The genuine remaining XFAIL is the
# production `setsockopt` path (LiteBox), which stays silent contract-free and is held behind
# `require_no_descriptor_writer`. That path's proxy writer state is only set by its synthetic
# wrapper; deriving directly from the real lock requires carrying the descriptor RwLock's instance
# identity across nested callbacks. It has no local reproduction here.
cargo test -p mirai --test integration_tests run_pass -- --exact
cargo build -p mirai --bin mirai

sysroot="$RUST_SYSROOT"
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

carrier_fixture="checker/tests/run-pass/arc_load_thin_pointer_known_limitation.rs"
carrier_output="$(
    target/debug/mirai \
        --crate-name mirai \
        "$carrier_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
printf '%s\n' "$carrier_output"

if ! grep -q 'warning: \[MIRAI\] unsatisfied precondition' <<<"$carrier_output"; then
    echo "The Arc-loaded callback carrier positive control did not fire." >&2
    exit 1
fi

# --diag=may-complete coverage-manifest invariant (Stage 1 "no-silent-unknown").
# This block force-uses the freshly built `target/debug/mirai` binary above so a stale
# binary cannot mask the exit-code invariant. It pins five load-bearing behaviours:
#   1. An incomplete coverage manifest MUST fail the process (nonzero exit).
#   2. The manifest MUST render clean:false and carry a region-axis gap marker.
#   3. A sealed, gap-free analyzed envelope MUST render clean:true and exit 0.
#   4. Crate timeout MUST leave the envelope unsealed.
#   5. Must-mode (--diag=paranoid) MUST emit no manifest and exit 0 (byte-unchanged).
coverage_fixture="checker/tests/run-pass/incomplete_summary_retains_precondition.rs"

set +e
may_complete_output="$(
    MIRAI_FLAGS="--diag=may-complete" target/debug/mirai \
        --crate-name mirai \
        "$coverage_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
may_complete_exit=$?
set -e
printf '%s\n' "$may_complete_output"

if [[ "$may_complete_exit" -eq 0 ]]; then
    echo "may-complete exit-code invariant failed: incomplete manifest exited 0." >&2
    exit 1
fi
if ! grep -q '"clean":false' <<<"$may_complete_output"; then
    echo "may-complete manifest did not render clean:false." >&2
    exit 1
fi
if ! grep -Eq '"(skipped_root|crate_timeout)"' <<<"$may_complete_output"; then
    echo "may-complete manifest carried no region-axis (skipped_root/crate_timeout) marker." >&2
    exit 1
fi

clean_coverage_fixture="checker/tests/run-pass/coverage_clean.rs"
clean_coverage_output="$(
    MIRAI_FLAGS="--diag=may-complete" target/debug/mirai \
        --crate-name coverage_clean \
        "$clean_coverage_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        2>&1
)"
if ! grep -q '"clean":true' <<<"$clean_coverage_output"; then
    echo "may-complete clean fixture did not render clean:true." >&2
    exit 1
fi
if ! grep -q '"outcome":"clean"' <<<"$clean_coverage_output"; then
    echo "may-complete clean fixture did not render a clean outcome." >&2
    exit 1
fi
if ! grep -q '"envelope_sealed":true' <<<"$clean_coverage_output"; then
    echo "may-complete clean fixture did not render a sealed envelope." >&2
    exit 1
fi

existential_fixture="checker/tests/run-pass/relaxed_precon.rs"
set +e
existential_output="$(
    MIRAI_FLAGS="--diag=may-complete" target/debug/mirai \
        --crate-name existential \
        "$existential_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
existential_exit=$?
set -e
if [[ "$existential_exit" -eq 0 ]]; then
    echo "existential abstract finding did not fail the process." >&2
    exit 1
fi
if ! grep -q '"tier":"abstract_counterexample"' <<<"$existential_output"; then
    echo "existential precondition check emitted no abstract counterexample." >&2
    exit 1
fi
if ! grep -q '"replay_validation":{"status":"undecided"}' <<<"$existential_output"; then
    echo "abstract counterexample did not render an explicit undecided replay status." >&2
    exit 1
fi
if ! grep -Eq '"witness":"[^"]+"' <<<"$existential_output"; then
    echo "existential precondition finding emitted no witness." >&2
    exit 1
fi
if grep -q '"witness":"model unavailable"' <<<"$existential_output"; then
    echo "existential precondition finding emitted a placeholder witness." >&2
    exit 1
fi

existential_unsat_fixture="checker/tests/run-pass/existential_unsat_complete.rs"
set +e
existential_unsat_output="$(
    MIRAI_FLAGS="--diag=may-complete" target/debug/mirai \
        --crate-name existential_unsat_complete \
        "$existential_unsat_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
existential_unsat_exit=$?
set -e
if [[ "$existential_unsat_exit" -eq 0 ]]; then
    echo "existential UNSAT fixture unexpectedly claimed globally complete coverage." >&2
    exit 1
fi
if ! grep -q '"tier":"solver_refutation"' <<<"$existential_unsat_output"; then
    echo "existential UNSAT fixture emitted no complete solver refutation." >&2
    exit 1
fi
if grep -Eq '"(abstract_counterexample|existential_check_undecided|existential_encoding_incomplete)"' <<<"$existential_unsat_output"; then
    echo "existential UNSAT fixture was not classified as a complete refutation." >&2
    exit 1
fi
if ! grep -q '"replay_validation_undecided"' <<<"$existential_output"; then
    echo "unvalidated existential model emitted no replay-undecided gap." >&2
    exit 1
fi
if grep -q '"tier":"abstract_model"' <<<"$existential_output"; then
    echo "existential model was promoted without concrete replay validation." >&2
    exit 1
fi

cargo build -p mirai --bin mirai --no-default-features --target-dir target/no-z3
set +e
existential_undefined_output="$(
    MIRAI_FLAGS="--diag=may-complete" target/no-z3/debug/mirai \
        --crate-name existential_no_z3 \
        "$existential_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
existential_undefined_exit=$?
set -e
if [[ "$existential_undefined_exit" -eq 0 ]]; then
    echo "no-Z3 existential undecided gap did not fail the process." >&2
    exit 1
fi
if ! grep -q '"existential_check_undecided"' <<<"$existential_undefined_output"; then
    echo "no-Z3 existential query emitted no undecided coverage gap." >&2
    exit 1
fi
if grep -q '"tier":"abstract_counterexample"' <<<"$existential_undefined_output"; then
    echo "no-Z3 existential query incorrectly emitted an abstract counterexample." >&2
    exit 1
fi
if grep -Eq '"witness":"[^"]+"' <<<"$existential_undefined_output"; then
    echo "no-Z3 existential query incorrectly emitted a witness." >&2
    exit 1
fi
if grep -q '"tier":"abstract_model"' <<<"$existential_undefined_output"; then
    echo "no-Z3 existential query incorrectly emitted a replay-confirmed model." >&2
    exit 1
fi

set +e
timeout_coverage_output="$(
    MIRAI_FLAGS="--diag=may-complete --crate_analysis_timeout 0" target/debug/mirai \
        --crate-name mirai_timeout \
        "$coverage_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
timeout_coverage_exit=$?
set -e

if [[ "$timeout_coverage_exit" -eq 0 ]]; then
    echo "may-complete timeout invariant failed: unsealed envelope exited 0." >&2
    exit 1
fi
if ! grep -q '"envelope_sealed":false' <<<"$timeout_coverage_output"; then
    echo "may-complete timeout invariant failed: crate timeout rendered a sealed envelope." >&2
    exit 1
fi
if ! grep -q '"crate_timeout"' <<<"$timeout_coverage_output"; then
    echo "may-complete timeout invariant failed: crate timeout marker was absent." >&2
    exit 1
fi

set +e
paranoid_coverage_output="$(
    MIRAI_FLAGS="--diag=paranoid" target/debug/mirai \
        --crate-name mirai \
        "$coverage_fixture" \
        --crate-type lib \
        --edition=2021 \
        -C debuginfo=2 \
        --out-dir "$output_dir" \
        --sysroot "$sysroot" \
        -Z span_free_formats \
        --extern "mirai_annotations=$annotations" \
        2>&1
)"
paranoid_coverage_exit=$?
set -e

if [[ "$paranoid_coverage_exit" -ne 0 ]]; then
    echo "must-mode invariant failed: --diag=paranoid exited nonzero on the coverage fixture." >&2
    exit 1
fi
if grep -q 'MIRAI_COVERAGE_MANIFEST' <<<"$paranoid_coverage_output"; then
    echo "must-mode invariant failed: --diag=paranoid emitted a coverage manifest." >&2
    exit 1
fi

echo "MIRAI test suite passed; both synthetic double-lock positive controls fired (direct-argument and Arc-loaded callback carrier). The production setsockopt path remains a documented XFAIL held behind require_no_descriptor_writer. The --diag=may-complete coverage manifest failed the process (exit ${may_complete_exit}) with a region-axis marker, the clean fixture exited 0 with clean:true and a sealed envelope, existential SAT stayed abstract with a replay-undecided gap, complete-encoding UNSAT and incomplete-encoding exclusion are unit-pinned, no-Z3 emitted an undecided gap without a witness, crate timeout left the envelope unsealed, and --diag=paranoid stayed byte-unchanged (no manifest, exit 0)."
