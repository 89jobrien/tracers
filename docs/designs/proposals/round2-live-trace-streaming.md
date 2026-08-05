---
title: Live trace streaming
slug: live-trace-streaming
round: 2
status: draft
viability: medium-low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Live trace streaming

## Problem

Trace<T> is only observable after an agent finishes — no way to see a long-running, multi-hop run in progress.

## Approach

`PartialUpdate<T>` enum streamed via a channel Agent::run pushes into internally; `Trace::stream()` yields updates ending in Finished(Trace<T>).

## API sketch

`enum PartialUpdate<T> { StepStarted { name: String }, StepCompleted { step: Step }, BranchRejected { branch: Branch }, Delegated { to: String }, Finished(Trace<T>) }`; `impl<T> Trace<T> { fn stream(self) -> impl Stream<Item = PartialUpdate<T>> }`

## Integration

run_with_escalation in tracers-runtime is the highest-value first wiring point — confirmed real in crates/runtime/src/execute.rs.

## Verification notes

Confirmed Agent::run's signature today returns Trace<Self::Output> directly with no channel/observer parameter — this is not purely additive as framed.

## Notes

Requires either changing AgentContext to carry an optional sender (touching every existing Agent impl) or a parallel path. Real signature-surgery cost, similar to trace-graph's problem. Decide cancellation semantics (does dropping the stream cancel the run?) explicitly.

## Prior art

- **OpenTelemetry Trace API / SDK spec** (https://opentelemetry.io/docs/specs/otel/trace/sdk/, https://docs.rs/opentelemetry_sdk/latest/opentelemetry_sdk/trace/struct.BatchSpanProcessor.html) — **this feature is genuinely differentiated, not a reinvention.** OTel's `SpanProcessor` interface has `on_start`/`on_end` hooks, but every standard exporter (`SimpleSpanProcessor`, `BatchSpanProcessor`) only ever exports a span once it has **ended** — there is no wire format or exporter contract anywhere in OTel for a span's partial/in-progress state. "Near-real-time" in OTel means low-latency delivery of *finished* spans, not incremental delivery of an unfinished one. A live-partial-state stream is real, novel ground relative to the industry-standard tracing model, confirmed directly against the spec rather than assumed.
- Because there's no existing streaming-partial-state convention to borrow patterns from, the cancellation-semantics and channel-plumbing design questions this doc already raises are the actual hard part — there's no off-the-shelf answer to crib from OTel or elsewhere.
