#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: a lock-effect boundary requires a reviewed contract and rejects
//! missing or body-derived summaries.
//!
//! NOTE: This is developer-reviewable lock-discipline *spec*, encoded with the existing
//! `mirai-annotations`/`contracts` surface. A lock-native MIRAI diagnostic (a lock-typed
//! MissingContract/BodyDerivedSummaryRejected) is the known, separately-scoped residual; today
//! MIRAI emits a generic precondition/verify violation, not a lock-typed one.

use contracts::*;
use mirai_annotations::*;

#[requires(
    has_reviewed_contract,
    "lock effect must come from a reviewed contract"
)]
fn require_reviewed_lock_contract(has_reviewed_contract: bool) {
    let _ = has_reviewed_contract;
}

fn missing_contract() {
    require_reviewed_lock_contract(false);
}

fn body_derived_summary() {
    require_reviewed_lock_contract(false);
}

fn reviewed_contract() {
    require_reviewed_lock_contract(true);
}

fn main() {
    reviewed_contract();
    if cfg!(mirai) {
        missing_contract();
        body_derived_summary();
    }
    println!(
        "missing_contract: spec encodes the provenance gate as a precondition violation \
         (generic, not lock-typed)"
    );
}
