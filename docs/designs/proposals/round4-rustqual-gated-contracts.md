---
title: rustqual-gated code-generation contracts
slug: rustqual-gated-contracts
round: 4
status: draft
viability: high
depends_on:
- contract-macro
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# rustqual-gated code-generation contracts

## Problem

A Coder agent has no structural way to guarantee its Rust output doesn't regress rustqual's quality score.

## Approach

A `Contract::post()` predicate that runs rustqual::check() against the generated diff and fails if the score regresses.

## API sketch

Worked instance of contracts_core::func::Contract — `.post(|code: &GeneratedCode| { let findings = rustqual::check(&code.diff_against(baseline)); if findings.score_delta() < 0.0 { Err(...) } else { Ok(()) } })`

## Integration

Depends on contract-macro (contracts-core::func) existing first — this is a worked example, not an independent mechanism.

## Verification notes

Confirmed rustqual is a real, installed binary (~/.cargo/bin/rustqual).

## Dependencies

- contract-macro

## Notes

rustqual likely takes real wall-clock time against a nontrivial diff — decide whether this blocks the agent's return (sync postcondition) or runs as a separate async-checked gate before implementing.

## Prior art
No dedicated research agent was run for this one — this is a worked instance of contract! (see
that design's own prior-art-relevant discussion, if any) applied to an already-verified local tool
(rustqual, confirmed real and installed). There's no external research question distinct from
contract!'s own design; the only open item (sync vs. async postcondition checking given rustqual's
real wall-clock cost) is an internal performance-engineering decision, not literature-answerable.
