// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// POSITIVE CONTROL (reclassified). This is a genuine double-lock: `acquire_write` sets the
// `writer` model field to 1 on the Arc-loaded socket-options lock, and the terminal callback
// calls `require_unlocked` while that writer is still held. Callback lineage carries the guarded
// model state through the nested `Option::map` re-wrap to the terminal callback, so MIRAI now
// DERIVES and reports the violation (see the expected-diagnostic marker below).
//
// History: this file was originally a known-limitation XFAIL (expected silent). The callback
// model-state carrier hops (commits 1d2aa9d..87fe37d) taught MIRAI to propagate the carried
// model field across the lifted callback, at which point the synthetic double-lock began to
// derive. It was flipped to a positive control at 87fe37d; the two out-of-band harnesses that
// still asserted silence (run_mirai_tests.sh, run_mirai_known_limits.ps1) were corrected to
// expect the diagnostic to fire.
//
// Sibling positive control: model_field_wrapper_field_double_lock.rs passes the owner as an
// explicit callback argument across the same Arc and wrapper-field hop, and also reports.
//
// ---------------------------------------------------------------------------------------------
// DOCUMENTED XFAIL (production `setsockopt`, no local reproduction — cross-repo, LiteBox).
// ---------------------------------------------------------------------------------------------
// The real production `setsockopt` double-lock is NOT auto-derivable by shipped MIRAI (b7b359f);
// the LiteBox manual contract `require_no_descriptor_writer(&self.litebox)` MUST be retained.
// Removing it leaves the production path silent. Three verified layers explain why clearing the
// callback-capture warnings is necessary but NOT sufficient:
//
//   1. Real lock state DOES exist on the path (correction to an earlier draft that claimed
//      "no state to carry"). On the real chain `RwLock::write` sets `write_held == 1` keyed to
//      the descriptor lock instance (LiteBox rwlock.rs:660-672 via litebox.rs:104-107,
//      net.rs:257), and the inner `RwLock::read` carries the precondition
//      `get_model_field!(self, write_held, 0) == 0` "read requires no live writer"
//      (LiteBox rwlock.rs:632-634 via litebox.rs:92-95, net/mod.rs:1516). The read precondition
//      IS attached and IS checked -- this is category (b): attached-but-not-refined-to-false,
//      NOT category (a) absence-of-precondition.
//   2. Detection is (also) deliberately proxy-decoupled from the real RwLock model. LiteBox
//      mirai_contracts.rs:1-7 states the feasibility rows consume the `&LiteBox`-level manual
//      contracts rather than the model annotations on `RwLock::read`/`write`/guard `Drop`. The
//      manual contract at LiteBox net.rs:408 is the SOLE contract-free precondition on the buggy
//      path; the proxy state `descriptor_write_held` is set only inside the synthetic
//      `mirai_with_descriptor_write` wrapper (net.rs:264), never on the real chain.
//   3. The production path has a genuine canonical-path identity split, but the A/B does not prove
//      that it is the sole cause. MIRAI keys lock model-state on the CANONICAL PATH of the handle
//      (checker/src/environment.rs:174, `canonicalize_model_field_path`; alias set stored at
//      :26/:147/:158, consumed at :189). If the outer `.write()` and inner `.read()` reach the
//      lock through handles that canonicalize to DIFFERENT paths (e.g. distinct `Arc<Lock>`
//      loads to the same underlying lock), the writer's `write_held == 1` is invisible to the
//      reader's `get_model_field!`. The production measurement below shows that annotating this
//      identity is not sufficient because the alias does not survive nested callback replay.
//
// The user's "alias on Arc" hypothesis maps to EXISTING machinery. MIRAI already provides
// `assumed_alias!(alias, source)`
// (annotations/src/lib.rs:430-442) -- "Declares that two referenced pointer-like values
// dereference to the same memory location [...] propagates this relationship to callers as part
// of the enclosing function's summary" -- consumed during callback-replay canonicalization
// (checker/src/environment.rs:174-205). Two in-tree positive controls prove it works for this
// shape (both VERIFIED firing against this checkout's target/debug/mirai):
//   * callback_alias_model_field_effect.rs -- two `Arc<Lock>` handles, write held across a
//     callback, inner read on the OTHER handle. WITH `assumed_alias!` (line 26) it fires
//     `unsatisfied precondition` (line 32); with the annotation removed it goes SILENT
//     (A/B verified). This is exactly the user's hypothesis working today.
//   * model_field_arc_alias.rs -- Arc-aliased model-field effect; fires.
//
// What was tried (hop-3, two independent implementations, same result). Field-decompose of the
// all-or-nothing capture gate at checker/src/block_visitor.rs:900-916 (line 908-909 keeps an
// argument only when neither path nor value contains a local variable; otherwise 913-914 sets
// `arguments_complete = false` and replaces the WHOLE argument with BOTTOM), plus field-granular
// dependency stripping in checker/src/call_visitor.rs (remove_unavailable_argument_dependencies
// def at :3892, called at :3456; report_unrepresentable_callback_argument called at :3887, def at
// :4162 emitting "callback argument could not be represented in summary" at :4169). Both the
// receiver-root probe and the full field-granular build
// cleared all three unrepresentable-argument warnings at both nested boundaries, yet the
// contract-free setsockopt A/B kill-switch stayed silent. Reverted per the time-box criterion.
// Conclusion: closure-capture representation is necessary but NOT sufficient -- the residual is
// state transport / canonical-path identity, not argument reconstruction.
//
// Production alias A/B (VERIFIED against the actual buggy `setsockopt`, not the separate
// `mirai_real_path_tripwire`, in `/workspace/litebox`): the descriptor
// lock is the direct `LiteBoxX.descriptors` field, but the outer write and inner read reach it
// through different `Arc<LiteBoxX>` clones (`GlobalState.litebox.x` and
// `Network.litebox.x`). Three placements were tested with `require_no_descriptor_writer` removed:
//   * clone-site: `assumed_alias!(&result.x, &self.x)` in `LiteBox::clone`;
//   * cross-crate call-frame helper after `self.net.lock()`;
//   * direct inline `assumed_alias!` in the selected shim `setsockopt` body.
// All remained silent. The direct inline probe removes summary availability as a confounder:
// MIRAI recognized the annotation in the selected function, but callback capture discarded the
// guard-local-rooted edge. The nested callback still contained `write_held == 1` and the real
// `read requires no live writer` precondition, while `pre_aliases` remained empty.
// Verbatim A/B evidence (contract removed):
//     === diagnostics ===
//     (empty — production `setsockopt` silent)
//     --- callback summary dump ---
//     pre_state:   [... "write_held", 1u ...]
//     pre_aliases: []
// Therefore a source annotation alone cannot replace the manual contract. The remaining gap is
// cross-HOF alias/model-state propagation in MIRAI, not merely an unannotated Arc clone.
//
// ---------------------------------------------------------------------------------------------
// FINAL A+B MEASUREMENT (converged close-out): the associated-type ICE guard, incomplete-summary
// retention/recovery, and lock-guard identity model make the production path measurable and retain
// both sides of the violation. A fresh two-pass persisted-summary run loaded 21 summaries. Its
// nested callback carried:
//     arguments_complete: false
//     pre_state: [... "write_held", 1u ...]
//     preconditions: [... "read requires no live writer" ...]
//     pre_aliases: []
// The unavailable callback argument is still represented as BOTTOM, but dependency filtering
// preserves the read precondition because its relevant roots remain available. Thus callback
// argument loss and cross-crate summary availability are no longer the terminal blockers.
//
// The remaining limitation is the absence of a sound caller-visible identity invariant between
// the two `Arc<LiteBoxX>` fields. `push_callback_invocation` intentionally retains only alias edges
// whose paths contain no local variables; a relationship rooted through the local mutex guard has
// no denotation in an arbitrary caller and cannot soundly be serialized by merely relaxing that
// filter. Supporting this production shape requires a new object-invariant or entry-replayed alias
// mechanism beyond A+B. Until then the real contract-free row remains XFAIL and
// `require_no_descriptor_writer` must remain in place.
//
// The `mirai_real_path_tripwire` is a different oracle: it invokes the LiteBox-level proxy
// contract through the same `&self.litebox` handle and exercises callback-capture representation.
// Its proxy-field/BOTTOM behavior neither supplies nor invalidates the actual `setsockopt` A/B
// above, which removed the proxy contract and observed the real RwLock `write_held` state.
//
// Citation provenance: all checker/src/* and annotations/src/* line numbers, and the two alias
// positive controls, are VERIFIED against this MIRAI checkout (b7b359f). The LiteBox citations
// and production A/B are VERIFIED in the separate `/workspace/litebox` checkout at b37c8c1a.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct Lock;

pub struct SocketOptions;

struct Inner {
    lock: Lock,
}

pub struct Owner {
    inner: Arc<Inner>,
}

impl Owner {
    fn lock(&self) -> &Lock {
        &self.inner.lock
    }

    fn acquire_write(&self) {
        let lock = self.lock();
        set_model_field!(lock, writer, 1usize);
    }

    fn require_unlocked(&self) {
        let lock = self.lock();
        precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
        //~ related location
    }

    fn socket_options_mut(&self) -> SocketOptionsGuard<'_> {
        self.acquire_write();
        SocketOptionsGuard {
            owner: self,
            options: SocketOptions,
        }
    }

    fn with_socket_options_mut<R>(
        &self,
        callback: impl FnOnce(&mut SocketOptions) -> R,
    ) -> R {
        self.socket_options_mut()
            .with_socket_options_mut(|options| callback(options))
    }

    pub fn trigger(&self) {
        self.with_socket_options_mut(|_options| {
            self.require_unlocked(); //~ unsatisfied precondition
        });
    }
}

struct SocketOptionsGuard<'a> {
    owner: &'a Owner,
    options: SocketOptions,
}

impl SocketOptionsGuard<'_> {
    fn with_socket_options_mut<R>(
        &mut self,
        callback: impl FnOnce(&mut SocketOptions) -> R,
    ) -> R {
        map_socket_options_mut(&mut self.options, |options| callback(options))
    }
}

impl Deref for SocketOptionsGuard<'_> {
    type Target = SocketOptions;

    fn deref(&self) -> &SocketOptions {
        &self.options
    }
}

impl DerefMut for SocketOptionsGuard<'_> {
    fn deref_mut(&mut self) -> &mut SocketOptions {
        &mut self.options
    }
}

impl Drop for SocketOptionsGuard<'_> {
    fn drop(&mut self) {
        set_model_field!(self.owner.lock(), writer, 0usize);
    }
}

fn map_socket_options_mut<R>(
    options: &mut SocketOptions,
    callback: impl FnOnce(&mut SocketOptions) -> R,
) -> R {
    Some(options).map(|options| callback(options)).unwrap()
}

pub fn main() {}
