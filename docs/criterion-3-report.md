# Criterion 3 Verification Report

## 1. Executive Summary

Criterion 3 is **PASS** on published branch `codex/arc-deref-model-clean`, anchored after `b1bcf49`; the decode oracle is durably committed in tested code ancestor `1a00076`. Both in-repo scopes verify green, backed by a cold-proven, committed fail→pass oracle. The sole residual is a non-gating external-LiteBox opaque-source item that does not reproduce in `CvvT/MIRAI`. No open, pending, or gating work remains.

## 2. Findings

### Scope 1 — Enum-lineage: PASS
Enum discriminant lineage tracking verified green. *(analyst/factchecker vetted; high confidence.)*

### Scope 2 (in-repo) — `u32::from_ne_bytes` decode boundary: PASS
The decode-boundary acceptance bar is met with a **genuine committed fail→pass oracle**: the **byte/shift cells** in `inferred_guarded_not_alias_precondition.rs`. *(Independently gated at exact `b1bcf49` by @factchecker — `run_pass` `test result: ok. 1 passed; 0 failed`, exit 0, byte- and shift-decoded cells each KEEPALIVE=1 / BROADCAST=0; verified, high confidence.)*

The teeth belong to two distinct cells in `inferred_guarded_not_alias_precondition.rs`, each fixed by a distinct commit:

| Cell | Silent (pre-fix) | Warns (after) | BROADCAST cell |
|---|---|---|---|
| **byte-decoded KEEPALIVE** (`u32::from_ne_bytes`) | under `ae56d9e` | after `47dc59f` | remains silent |
| **shift-decoded KEEPALIVE** (`<<`/`|` reconstruction) | before `b1bcf49` | after `b1bcf49` | remains silent |

`47dc59f` adds the `from_ne_bytes` decode-lineage fix; `b1bcf49` adds **shift reconstruction** (not another `from_ne_bytes` regression). Independently confirmed three ways — @opus's cold-built per-cell rebuild, @codex's commit provenance (`ae56d9e`→`47dc59f`, exit 101→0), and @factchecker's byte-level gate (`BYTE_KEEPALIVE=1`, `BYTE_BROADCAST=0`, `run_pass exit=0`).

**Admissibility confirmed (@opus three-cell disambiguator, single-cell annotated method).** A control cell that decodes via `from_ne_bytes` then issues an **unconditional** require (no `==9` guard) **warns at `ae56d9e`** — proving the alias obligation *does* form through the byte transform (MIRAI models the decoded value). The guarded byte-decoded cell is **silent at `ae56d9e` → warns after `47dc59f`**. So the pre-fix miss is an **admissible in-repo false negative** (discriminant lost only at the `==9` guard), **already closed** by the committed fix — not an inadmissible modeling gap. This admissibility control (`unconditional_byte_decoded_alias`) is now **durably committed** to the tracked regression at `1a00076` (full `run_pass` suite green, 263 fixtures, exit 0).

## 3. Corrections of Record

- **`transport_decode_probe.rs` (`2c42572`) is corroborating coverage only — NOT fail→pass.** It warns identically at *both* `ae56d9e` and the published post-fix branch (KEEPALIVE 1, BROADCAST 0); constant-folding explains it. It is a faithful-shape guard with no teeth and is **not** grouped into the fail→pass evidence. The load-bearing fail→pass evidence is solely the byte-decoded and shift-decoded KEEPALIVE cells above.
- **The "decoded-silent / literal-warns" matched-plumbing isolation is NOT load-bearing.** @factchecker's matched-control (`matched.rs`) and opaque-pair runs both showed *no* decode-vs-direct divergence (both fire, exit 0). That specific isolation does not discriminate; the committed byte/shift fail→pass is the decisive evidence.
- **`ae56d9e` is the byte-cell pre-fix baseline only** (parent of `47dc59f`) — it is never the closure anchor. Closure is anchored at `b1bcf49+`.

## 4. Closure Rationale

Criterion 3 = PASS on the published branch, with the tested code closure at ancestor `1a00076`, rests on two committed per-cell fail→pass oracles in `inferred_guarded_not_alias_precondition.rs`, each observed via the committed **single-cell annotated `run_pass`** method (@opus cold-built, @codex commit-provenance): the **byte-decoded KEEPALIVE** cell (silent under `ae56d9e`, warns after `47dc59f`) and the **shift-decoded KEEPALIVE** cell (silent before `b1bcf49`, warns after `b1bcf49`), with the corresponding BROADCAST cells staying silent. (This silent→warn attribution is per-cell under the annotated method; it is distinct from @factchecker's *matched-plumbing* `matched.rs` control, which showed no decode-vs-direct divergence — see §3, and is deliberately excluded from the load-bearing evidence.) It does **not** rest on `transport_decode_probe.rs` (corroborating shape guard only, warns both pre- and post-fix) and does **not** rest on the matched-plumbing decoded-vs-literal isolation (which showed no divergence — both fire, exit 0).

### Residual — External-LiteBox opaque pointer (Item 3): non-gating
The external-LiteBox opaque `read_from_user::<u32>` pointer shape (no concrete bytes to propagate) remains a documented, non-gating backlog item; it does not reproduce in `CvvT/MIRAI`. The pre-existing 2021 `body_visitor.rs:4061` union ICE is resolved by `1990510`, with nested-union regression coverage in `union_field_assignment.rs`. No LiteBox stack-overflow signature is substantiated by any captured artifact (the most recent external capture is a distinct rustc trait-resolution ICE in `try_to_devirtualize`, not stack exhaustion); any such external-only instability, if later reproduced, is orthogonal to Criterion 3's in-repo PASS and non-gating.

### Known limitation — LiteBox `setsockopt` path (XFAIL)
As verified at anchor `43666db`, Scope 2's load-bearing fail→pass oracle covers the byte/shift-decode value-lineage cells only (KEEPALIVE warns / BROADCAST silent, direct-driver verified); Criterion 3's overall PASS additionally covers Scope 1 and the corroborating transport shape guard. The real LiteBox production `setsockopt` path remains a documented **XFAIL** — it does not reproduce in `CvvT/MIRAI`, so the manual `require_no_descriptor_writer` contract is still required for that path.

### Known limitation — load-induced false negatives (non-gating, environmental)
Under pathological host load (observed only at load average ≥~26; absent at ≤12), MIRAI's timeout-bounded analysis can **drop** expected diagnostics — a false *negative*, not a false positive. Two independent, orthogonal timeouts contribute: the 100 ms per-Z3-query wall-clock timeout (`z3_solver.rs:67`) yielding `SmtResult::Undefined`, and the per-function analysis budget (`BodyTimeout`). This is a global CPU-starvation artifact affecting generic fixtures (e.g. `arc_load_thin_pointer`, `model_field_arc_alias`, `callback_carrier`) and is **not** a soundness gap in the socket-decode gate: the Criterion 3 decode cells route through the pre-solver, deterministic `EncodingIncomplete` path (`body_visitor.rs`, `existential_encoding_is_complete`) and are timeout-invariant by construction — they never regressed under load. A per-query high-timeout re-solve on `Undefined` was implemented and load-tested (@opus); it did **not** eliminate the flakiness (15/20 under synthetic load 26–62) because it cannot address the orthogonal `BodyTimeout` and can worsen it by consuming the body budget, so it was reverted on canonical (`317806b`). Recorded as a documented environmental limitation, not a Criterion 3 regression. The direction is dictated by construction: every `Undefined` arm is non-emitting (`call_visitor.rs:5289` is `=> continue`), so a timeout can only drop a true positive, never add a spurious note.

### Hygiene at the verification anchor
At verification, canonical `/workspace/MIRAI` matched the published origin tip with empty status. Tested code ancestor `1a00076` contained the durable admissibility control, union-field ICE hardening (`1990510`) was present in the verified branch, and `run_pass` completed green. Later worktree state does not alter these commit-scoped results.

### Solver-timeout false positive: resolved (`23d186a`)
The solver-timeout false-positive path is **resolved**, not open: `23d186a` "Separate solver timeouts from incomplete encoding" distinguishes `Solver(SmtResult::Undefined)` (timeout — skips diagnostic emission) from `EncodingIncomplete` (retains the guarded diagnostic), covered by the committed test `complete_boolean_query_distinguishes_incomplete_encoding_from_solver_timeout` in `body_visitor.rs`. @factchecker's under-load loop ran 30/30 stable.

## 5. Conclusion

Criterion 3 is fully verified and permanently closed on the published branch, with tested code closure at ancestor `1a00076` (after `b1bcf49`). Both in-repo scopes pass, the decode boundary is backed by a real discriminating oracle (byte/shift cells) proven across a cold-built pre-fix checker, and only a non-reproducing external opaque-source item remains — non-gating.

## 6. Sources
- Cold fail→pass differentials on the two per-cell oracles: byte-decoded KEEPALIVE (`ae56d9e` → `47dc59f`) and shift-decoded KEEPALIVE (pre-`b1bcf49` → `b1bcf49`): @opus, @codex
- Byte-level per-cell gate + positive-control classification: @factchecker
- `transport_decode_probe.rs` corroborating only / non-discriminating (warns pre- and post-fix): @factchecker, @opus
- Load-induced false-negative characterization (15/20 under synthetic load 26–62; failures are dropped warnings / body-level timeouts on generic fixtures, never the decode cells) and reverted re-solve experiment: @opus
- Terminal disposition and standing rulings: @lead
- Commits: `3b697a8` (original verification-report commit), `1a00076` (tested code closure — durable admissibility control cell `unconditional_byte_decoded_alias`), `9ed4df3` (rustfmt), `47dc59f` (byte-decode `from_ne_bytes` oracle + fix), `b1bcf49` (shift-reconstruction fix + oracle — not another `from_ne_bytes` regression), `2c42572` (corroborating shape guard `transport_decode_probe.rs`), `1990510` (union-field ICE hardening), `23d186a` (solver-timeout vs incomplete-encoding separation), `317806b` (revert of the speculative one-shot Z3 re-solve — load-tested, did not remove the environmental false negatives). `ae56d9e` = byte-cell pre-fix baseline only (parent of `47dc59f`); closure anchored at `b1bcf49+`.
