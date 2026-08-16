# Changelog

All notable changes to this project are documented in this file.

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


