# trace:: fuzz targets

The workspace-wide invariant CLAUDE.md states — *all types are
`Serialize + Deserialize`, as a compile-time constraint* — is exactly the kind
of claim a fuzzer is good at falsifying. These targets attack it from both
directions: build a value and prove it survives a round trip, and feed the
parsers arbitrary bytes and prove they fail rather than panic.

## Running

Requires nightly and `cargo-fuzz`:

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run trace_roundtrip
cargo +nightly fuzz run checkpoint_parse
```

The crate is excluded from the main workspace (see `exclude` in the root
`Cargo.toml`) because cargo-fuzz builds with nightly `-Z` flags the rest of the
workspace neither needs nor should inherit.

## Targets

| target | invariant |
| --- | --- |
| `trace_roundtrip` | a `Trace<String>` built from arbitrary input serializes, deserializes, and re-serializes to byte-identical JSON |
| `checkpoint_parse` | `TaskRegistry` and `Trace` reject arbitrary bytes with an error, never a panic — a corrupt checkpoint on disk must not take a pipeline down |

A crash writes a reproducer under `artifacts/`; replay it with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`.
