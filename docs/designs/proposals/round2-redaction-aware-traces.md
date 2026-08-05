---
title: Redaction-aware traces
slug: redaction-aware-traces
round: 2
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Redaction-aware traces

## Problem

A checkpointed or exported Trace<T> may carry secrets/PII; nothing distinguishes a trace safe to share from one that needs scrubbing.

## Approach

Phantom `Sensitivity` marker (`Trace<T, S = Unclassified>`); `Trace<T, Sensitive>::export_redacted(level: obfsck::Level)` is the *only* export path for a Sensitive trace.

## API sketch

`struct Trace<T, S = Unclassified>`; marker types `Sensitive`/`Public`/`Unclassified`; `impl<T: Serialize> Trace<T, Sensitive> { fn export_redacted(&self, level: obfsck::Level) -> Result<String, TraceErr> }`

## Integration

Wires into obfsck::ObfuscationLevel — confirmed real, with Minimal/Standard/Paranoid variants and actual redaction logic in ~/dev/obfsck/src/lib.rs.

## Verification notes

Confirmed obfsck is a real, working crate with the exact enum/levels claimed.

## Notes

Lower risk than trust-provenance's phantom-type approach since Sensitivity only gates an export method — doesn't need to survive TaskStatus/TraceRef persistence, so no erasure problem.

## Prior art
No dedicated research agent was run for this one. PII/secret redaction is a mature, well-covered
engineering domain (differential privacy, data-loss-prevention tooling, log-scrubbing pipelines)
but this proposal's actual design content is entirely about wiring a phantom-type export gate to
an already-existing, already-verified local tool (obfsck) — there's no external research question
here, the relevant "prior art" is the obfsck crate itself, already confirmed real in this doc's
Verification notes.
