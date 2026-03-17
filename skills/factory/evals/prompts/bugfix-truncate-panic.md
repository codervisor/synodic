# Eval: Bugfix — truncate() panics on multi-byte UTF-8

Run the factory skill on the truncate panic bugfix spec:

```
/factory run skills/factory/fixtures/bugfix-truncate-panic/README.md
```

## Why this is hard

- Requires reading and understanding existing Rust code in `station.rs`
- The fix must preserve existing byte-bounding semantics (not switch to char count)
- Unit tests must exercise the exact panic scenario (multi-byte char boundary)
- `cargo test` and `cargo clippy` must pass — the compiler enforces correctness

## Verify

1. `truncate()` in `station.rs` no longer uses `&s[..max]` (the panic-prone pattern).
2. The fix uses `str::floor_char_boundary()`, `char_indices()`, or equivalent safe approach.
3. At least 5 unit tests exist covering the cases in the spec.
4. `cargo test --workspace` passes.
5. `cargo clippy --workspace` has no warnings.
6. The function signature is unchanged.
