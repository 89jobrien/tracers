---
title: Trace::narrate()
slug: trace-narrate
round: 2
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace::narrate()

## Problem

causal_chain() gives structured data; a human audience needs a readable sentence, and paying for an LLM call just to summarize data trace:: already has is wasteful.

## Approach

Deterministic, template-based renderer over Step/Branch — no LLM call, every fact traces back to a field already on Step or Branch.

## API sketch

`impl<T: Clone + Serialize> Trace<T> { fn narrate(&self) -> String }`

## Integration

Pure trace-core addition, no new crate, no dependency. Composes for free with redaction-aware-traces and view-as-audience.

## Verification notes

Confirmed StepOutcome::Taken/Rejected/Failed and BranchOutcome::Taken/Rejected match the sketch's match arms exactly (crates/core/src/step.rs).

## Notes

Ships in under an hour — lower effort than even step-cost-ledger since there's no new struct, just a method.

## Prior art

**OpenTelemetry has no equivalent to this feature at all** — confirmed via direct comparison of the OTel Span data model (name, SpanContext, SpanKind, timestamps, flat attribute bag, Events, Links, a 3-valued Status enum) against `Step`/`Branch`. Two fields this proposal (and trace-core generally) relies on have no OTel representation whatsoever: `confidence` (no first-class concept anywhere in the spec — would have to be an ad hoc numeric attribute, as e.g. the vLLM Semantic Router project does informally) and `Branch` — a considered-but-rejected alternative. OTel's `Links` connect spans causally but express nothing like "this was considered and not taken." So `narrate()`'s value (rendering confidence scores and rejected-alternative reasoning in plain English) is templating over data that genuinely doesn't exist in the industry-standard tracing model — real differentiation, not a reinvention of an OTel convention.
