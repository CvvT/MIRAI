#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: acquiring cache before database violates the reviewed
//! receiver-local lock order.
//!
//! NOTE: This is developer-reviewable lock-discipline *spec*, encoded with the existing
//! `mirai-annotations`/`contracts` surface. A lock-native MIRAI diagnostic (a lock-typed
//! DoubleLock/OrderViolation/UnheldRelease) is the known, separately-scoped residual; today
//! MIRAI emits a generic precondition/verify violation, not a lock-typed one.

use contracts::*;
use mirai_annotations::*;

#[requires(!database_held, "database lock must not already be held")]
#[requires(
    !cache_held || cache_before_database,
    "reviewed order edge cache < database is required"
)]
#[ensures(ret, "database lock is held after acquire")]
fn acquire_database(database_held: bool, cache_held: bool, cache_before_database: bool) -> bool {
    let _ = (database_held, cache_held, cache_before_database);
    true
}

#[requires(!cache_held, "cache lock must not already be held")]
#[requires(
    !database_held || database_before_cache,
    "reviewed order edge database < cache is required"
)]
#[ensures(ret, "cache lock is held after acquire")]
fn acquire_cache(cache_held: bool, database_held: bool, database_before_cache: bool) -> bool {
    let _ = (cache_held, database_held, database_before_cache);
    true
}

fn lock_order_inversion() {
    let database_held = false;
    let cache_held = false;
    let database_before_cache = true;
    let cache_before_database = false;

    let cache_held = acquire_cache(cache_held, database_held, database_before_cache);
    let _database_held = acquire_database(database_held, cache_held, cache_before_database);
}

fn main() {
    if cfg!(mirai) {
        lock_order_inversion();
    }
    println!(
        "lock_order_inversion: spec encodes cache-before-database as a precondition violation \
         (generic, not lock-typed)"
    );
}
