# RwLock annotation detection

This example uses stock MIRAI model fields to shadow a sequential read/write lock protocol:

- `read_count: usize` counts simultaneously live read guards on the analyzed path.
- `write_held: usize` uses `0/1` to record a live writer.
- `Guard::drop` and `WriteGuard::drop` release their corresponding mode.

The fixtures are independent binaries:

| Binary | Expected result |
| --- | --- |
| `clean` | no MIRAI diagnostics |
| `read_then_write` | `write requires no live readers` |
| `unheld_release` | `read release requires a live reader` |
| `drop_then_release` | `read release requires a live reader`; proves reader `Drop` decremented |
| `double_write` | `write requires no live writer` with the checker fix |
| `write_then_read` | `read requires no live writer` |
| `write_drop_then_reacquire` | silent; write `Drop` permits a later read and write |
| `two_readers_drop_one` | `write requires no live readers`; one reader remains live |
| `two_readers_drop_both` | silent; both reader releases survive summary arithmetic |
| `instances_independent` | silent; a reader on `a` does not block a writer on `b` |
| `seeded_writer_other_instance` | silent; known writer state on `a` does not block reading `b` |
| `instance_violation` | `write requires no live writer` on `a`, while `b` remains unlocked |
| `alias_same_instance` | `write requires no live readers`; `&a` shares `a`'s state |
| `array_indices` | silent; constant indices `locks[0]` and `locks[1]` remain distinct |
| `array_same_index` | `write requires no live readers`; repeated `locks[0]` is the same instance |
| `callback_inline_closure` | `write requires no live writer` |
| `callback_accessor_violation` | `read requires no live writer`; callback pre-state survives a `&`-returning accessor |
| `callback_hof_annotated` | `write requires no live writer` |
| `callback_hof_invoke_twice` | `annotated callback requires no live writer` |
| `callback_hof_direct_control` | `annotated callback requires no live writer` |
| `callback_fnptr` | `write requires no live writer` |
| `callback_reentrant` | `write requires no live writer` |
| `callback_clean` | no MIRAI diagnostics |
| `callback_conditional_false` | silent; the guarded callback is not invoked |
| `callback_conditional_true` | `conditional callback requires no live writer` |
| `callback_generic_fnonce_clean` | silent; generic `FnOnce` summary resolution |
| `callback_generic_fnonce_violation` | `read requires no live writer` |
| `callback_sequential_counter_clean` | silent; second callback observes `read_count == 1` |
| `callback_sequential_counter_violation` | second callback's incorrect `read_count == 2` requirement fires |
| `callback_specialization_clean` | silent; two closure specializations stay isolated |
| `callback_specialization_violation` | `read requires no live writer` for the violating closure only |
| `callback_unresolvable` | visible `callback invocation could not be resolved` diagnostic |
| `callback_loop_clean` | silent; asymmetric fixed-point summary retains a clean invocation |
| `callback_loop_violation` | summary-only `read requires no live writer`; union-retained asymmetric invocation |
| `callback_captured_nested_clean` | silent; a captured inner callback runs after the modeled writer is released |
| `callback_captured_nested_violation` | `read requires no live writer`; nested replay preserves the captured lock identity |
| `callback_multi_hop_clean` | silent; transitive closure specialization survives three HOF layers |
| `callback_multi_hop_violation` | `read requires no live writer`; transitive closure specialization survives three HOF layers |
| `callback_nested_hof_clean` | silent negative control: callback passes through an adapter HOF and runs after the writer is released |
| `callback_nested_hof_violation` | `read requires no live writer`; callback passes through an adapter HOF while the writer remains held |
| `callback_fnptr_specialization_clean` | silent; bare function-pointer control |
| `callback_fnptr_specialization_violation` | `read requires no live writer`; same-typed `clean`/`read` pointers stay isolated |
| `nested_field_double_write` | `write requires no live writer` |
| `nested_field_independent` | no MIRAI diagnostics |
| `nested_deep_double_write` | `write requires no live writer` |
| `nested_deep_independent` | no MIRAI diagnostics |
| `struct_field_double_write` | `write requires no live writer` |
| `struct_fields_independent` | no MIRAI diagnostics; distinct fields remain distinct |
| `arc_alias_double_write` | `write requires no live writer`; cloned handles share one pointee |
| `arc_clone_or_fresh_false` | no MIRAI diagnostics; the selected branch returns a fresh allocation |
| `arc_clone_or_fresh_true` | `write requires no live writer`; the selected branch clones the input |
| `arc_clone_wrapper_double_write` | `write requires no live writer`; an unconditional wrapper preserves the clone alias |
| `arc_instances_independent` | no MIRAI diagnostics |
| `rc_alias_double_write` | `write requires no live writer`; cloned handles share one pointee |
| `rc_instances_independent` | no MIRAI diagnostics |

Unpatched stock MIRAI considers the continuation after `acquire_write` unreachable because it
promotes the stale `writer == 0` precondition into a postcondition. The checker fix excludes
body-mutated model fields from automatically inferred postconditions, so the second ordinary
acquisition is now reached and rejected.

Model fields are qualified by the receiver path, not keyed only by type or acquisition site.
`handle_get_model_field` and `handle_set_model_field` build and canonicalize
`receiver.model_field(name)` paths. This distinguishes separate locals and constant array elements
while canonicalizing a direct reference alias back to the same instance. Runtime-selected or
imprecisely indexed collections are not covered by these fixtures.

The wrapper models sequential acquisition discipline, not thread interleavings or the behavior of
`std::sync::RwLock`. The clean result refers to MIRAI's default diagnostic policy; paranoid mode
also reports possible reader-count overflow and read-release imprecision.

The mode/RAII and higher-order fixtures can also consume persisted provider summaries without
entering the provider method or callback bodies. This proves that write/read acquisition, both
guard releases, `read_count + 1`/`read_count - 1`, and callback invocations under intermediate
model-field state survive the summary boundary. Other rows still use MIRAI's top-down body analysis
and do not establish strict contract-only modularity.

Run the complete sweep with raw output:

```powershell
.\examples\rwlock_annotation_detection\run_examples.ps1 -ShowOutput
```

Use `-Filter <binary>` to run one fixture. Every violating row requires exactly one matching MIRAI
diagnostic, so a missed alias or a duplicate diagnostic makes the runner exit nonzero.
The `Arc`/`Rc` rows load MIRAI's embedded standard contracts; other rows start with an empty summary
store to retain their original standalone-analysis oracle.

Run the mode/RAII and higher-order fixtures against persisted summaries:

```powershell
.\examples\rwlock_annotation_detection\run_examples.ps1 -SummaryOnly
```

This mode seeds provider summaries, recompiles each consumer, and fails if MIRAI enters a protected
provider, higher-order helper, or callback body.

Direct bare function-pointer calls are recorded from their ordinary indirect MIR `Call`, while
concrete callback identity remains specialized at each consumer call site. The clean/violation pair
uses same-typed function pointers to ensure their obligations do not contaminate each other.

Run the former limitation probe explicitly as a focused regression:

```powershell
.\examples\rwlock_annotation_detection\run_examples.ps1 -KnownLimitations
```

The shared-store standard-summary load gate has a separate ignored tripwire:

```powershell
cargo test -p mirai --lib --no-default-features summaries::tests::shared_store_seeds_embedded_standard_summaries -- --ignored --exact
```

It is intentionally red until an empty shared store is seeded from the embedded standard-summary
archive. The paired `callback_nested_hof_{clean,violation}` examples isolate MIRAI's
HOF-through-HOF mechanism, while LiteBox's `real_path_incomplete_summary` remains the real-path
integration oracle. Both nested examples invoke the callback through the same adapter boundary; the
clean control releases its writer before invocation, while the violation keeps its writer live.
Replay resolves the adapter with its full function-constant signature and overlays the recorded
invocation-site model state, so the clean callback stays silent while the violation reports
`read requires no live writer`.

Adapter-closure specialization lookup has a focused unit regression:

```powershell
cargo test -p mirai --lib --no-default-features summaries::tests::adapter_closure_specialization_is_replay_resolvable -- --exact
```

It mirrors layer 2: summarization and replay both key the adapter with
`[adapter, user callback]`, preserving the concrete callback specialization across the shared store.

Callback effects are checked at each recorded snapshot. Replay does not accumulate effects between
snapshots, which prevents double-applying arithmetic model fields such as `read_count`.

Invocation guards retain only conditions expressible through parameters and model fields. Guards
that depend on helper-local control state conservatively become `true`; this can add false positives
but cannot hide a callback obligation. LiteBox's buggy call is unconditional inside its closure,
while the fixed call is outside the helper, so this boundary does not affect the target comparison.
