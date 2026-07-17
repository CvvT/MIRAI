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
| `callback_hof_annotated` | `write requires no live writer` |
| `callback_hof_invoke_twice` | `annotated callback requires no live writer` |
| `callback_hof_direct_control` | `annotated callback requires no live writer` |
| `callback_fnptr` | `write requires no live writer` |
| `callback_reentrant` | `write requires no live writer` |
| `callback_clean` | no MIRAI diagnostics |
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

The four mode/RAII fixtures can also consume persisted provider summaries without entering the
provider method bodies. This proves that write/read acquisition, both guard releases, and
`read_count + 1`/`read_count - 1` survive the summary boundary. Other rows still use MIRAI's
top-down body analysis and do not establish strict contract-only modularity. In particular,
ordinary summaries do not represent a callback invocation under an intermediate lock state, so
the higher-order propagation requirement needs a separate checker design.

Run the complete sweep with raw output:

```powershell
.\examples\rwlock_annotation_detection\run_examples.ps1 -ShowOutput
```

Use `-Filter <binary>` to run one fixture. Every violating row requires exactly one matching MIRAI
diagnostic, so a missed alias or a duplicate diagnostic makes the runner exit nonzero.
The `Arc`/`Rc` rows load MIRAI's embedded standard contracts; other rows start with an empty summary
store to retain their original standalone-analysis oracle.

Run the four mode/RAII fixtures against persisted summaries:

```powershell
.\examples\rwlock_annotation_detection\run_examples.ps1 -SummaryOnly
```

This mode seeds provider summaries, recompiles each consumer, and fails if MIRAI enters a provider
`read`, `write`, release, or guard `Drop` body.
