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
The external-LiteBox opaque `read_from_user::<u32>` pointer shape (no concrete bytes to propagate) remains a documented, non-gating backlog item; it does not reproduce in `CvvT/MIRAI`. The pre-existing 2021 `body_visitor.rs:4061` union ICE is resolved by `1990510`, with nested-union regression coverage in `union_field_assignment.rs`; the separate LiteBox stack overflow remains non-gating.

### Hygiene: clean
Canonical `/workspace/MIRAI` matched the published origin tip with empty status at verification. Tested code ancestor `1a00076` contains the durable admissibility control. Union-field ICE hardening (`1990510`) is present in the published branch, and `run_pass` is green. Stale-SHA/dirty banners from frozen session worktrees are known artifacts, not regressions.

### Solver-timeout false positive: resolved (`23d186a`)
The solver-timeout false-positive path is **resolved**, not open: `23d186a` "Separate solver timeouts from incomplete encoding" distinguishes `Solver(SmtResult::Undefined)` (timeout — skips diagnostic emission) from `EncodingIncomplete` (retains the guarded diagnostic), covered by the committed test `complete_boolean_query_distinguishes_incomplete_encoding_from_solver_timeout` in `body_visitor.rs`. @factchecker's under-load loop ran 30/30 stable.

## 5. Conclusion

Criterion 3 is fully verified and permanently closed on the published branch, with tested code closure at ancestor `1a00076` (after `b1bcf49`). Both in-repo scopes pass, the decode boundary is backed by a real discriminating oracle (byte/shift cells) proven across a cold-built pre-fix checker, and only a non-reproducing external opaque-source item remains — non-gating.

## 6. Sources
- Cold fail→pass differentials on the two per-cell oracles: byte-decoded KEEPALIVE (`ae56d9e` → `47dc59f`) and shift-decoded KEEPALIVE (pre-`b1bcf49` → `b1bcf49`): @opus, @codex
- Byte-level per-cell gate + positive-control classification: @factchecker
- `transport_decode_probe.rs` corroborating only / non-discriminating (warns pre- and post-fix): @factchecker, @opus
- Terminal disposition and standing rulings: @lead
- Commits: `3b697a8` (original verification-report commit), `1a00076` (tested code closure — durable admissibility control cell `unconditional_byte_decoded_alias`), `9ed4df3` (rustfmt), `47dc59f` (byte-decode `from_ne_bytes` oracle + fix), `b1bcf49` (shift-reconstruction fix + oracle — not another `from_ne_bytes` regression), `2c42572` (corroborating shape guard `transport_decode_probe.rs`), `1990510` (union-field ICE hardening), `23d186a` (solver-timeout vs incomplete-encoding separation). `ae56d9e` = byte-cell pre-fix baseline only (parent of `47dc59f`); closure anchored at `b1bcf49+`.
