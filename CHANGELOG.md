# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Documentation

- Add a runnable examples crate

### Features

- Add step cost ledger
- Add TraceGraph for cross-trace lineage
- Add Contract step pre/post-conditions
- Add pausable, resumable traces for human-in-the-loop
- Add speculate_race early-exit combinator
- Add trace-cli, a checkpoint inspector

### Miscellaneous

- Enable post-auto push of main and flow branches
- Rust-audit mechanical fixes - derives, docs, crate metadata
- Clear the remaining configuration and CI TODOs

### Testing

- Add criterion benches and cargo-fuzz targets

### Flow

- Promote develop into staging
- Stage staging into release
- Finish release into main
- Sync main into develop
- Promote develop into staging
- Stage staging into release
- Finish release into main

## [0.2.1] - 2026-08-18

### Bug Fixes

- Make self dev-dependency path-only so first publish can resolve

## [0.2.0] - 2026-08-17

### Features

- Rename crates to trace-lang-* for crates.io publish

## [0.1.1] - 2026-08-17

### Documentation

- Update handoff

## [0.1.0] - 2026-08-16

### Bug Fixes

- Factor speculate test candidate type to satisfy clippy type_complexity
- Bump tokio to 1.44.2, add dual MIT/Apache-2.0 LICENSE files
- Re-export StepOutcome; chore(deny): trim stale license allowances

### Documentation

- Add doc comments to raise public API doc coverage
- Add per-crate READMEs, fix related-work reference
- Add CONTRIBUTING.md, issue templates, and PR template
- Flag self-dependency needs a version before first publish

### Features

- Derive miette::Diagnostic on TraceErr
- Expose CheckpointStore conformance suite via test-support feature
- Route taskit's SARIF output to GitHub code scanning
- Add mise.toml for tool provisioning and composite workflows
- Add test-support feature, promote async-trait to optional dep
- Scaffold new crate and register in workspace
- Implement TraceOutcome port for SpawnOutcome/RunOutcome
- Implement TraceAssertionError and the four assertion primitives
- Implement assert_trace! macro

### Refactor

- Move escalation_wiring fixtures into src/fixtures.rs

### Testing

- Add CheckpointStore conformance suite, apply to FileCheckpointStore
- Grow test suite from 12 to 41 unit tests
- Fill testing-philosophy gaps — property, model check, integration
- Expand coverage on every testing-philosophy dimension
- Add property test for confidence_below
- Add integration test against the real escalation chain fixture


