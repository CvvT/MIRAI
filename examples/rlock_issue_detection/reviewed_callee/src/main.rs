#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: the call boundary uses the reviewed callee contract instead of
//! a body-derived lock summary.
//!
//! NOTE: This is developer-reviewable lock-discipline *spec*, encoded with the existing
//! `mirai-annotations`/`contracts` surface. A lock-native MIRAI diagnostic is the known,
//! separately-scoped residual; today MIRAI verifies the reviewed callee contract via generic
//! ensures/verify, not a lock-typed effect. Per the Verus-style read rule, the caller reads
//! only the callee's reviewed contract, never its body.

use contracts::*;
use mirai_annotations::*;

#[ensures(ret, "reviewed contract: callee acquires database")]
fn reviewed_acquire_database() -> bool {
    true
}

#[allow(dead_code)]
#[ensures(ret, "body-derived summary: callee acquires disk")]
fn body_derived_acquire_disk() -> bool {
    true
}

fn caller_reads_reviewed_contract_only() {
    let disk_held = false;
    let database_held = reviewed_acquire_database();

    verify!(database_held);
    verify!(!disk_held);
}

fn main() {
    caller_reads_reviewed_contract_only();
    println!(
        "reviewed_callee: spec verifies the reviewed callee contract only \
         (generic ensures/verify, not lock-typed)"
    );
}
