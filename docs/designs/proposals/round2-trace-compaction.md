---
title: Trace compaction
slug: trace-compaction
round: 2
status: draft
viability: medium
depends_on:
- trace-archive
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Trace compaction

## Problem

A long-running/multi-hop trace's causal chain keeps growing and never shrinks; most of it is noise for debugging or archival purposes.

## Approach

`RetentionPolicy` (keep_failures, keep_rejected_branches, keep_last_n_success, confidence_floor); `Trace::compact()` folds non-kept steps into a `CompactedSummary`.

## API sketch

`struct RetentionPolicy { keep_failures: bool, keep_rejected_branches: bool, keep_last_n_success: usize, confidence_floor: Option<f64> }`; `impl<T> Trace<T> { fn compact(&mut self, policy: RetentionPolicy) }`; `struct CompactedSummary { step_count: usize, time_range: (DateTime<Utc>, DateTime<Utc>), mean_confidence: Option<f64> }`

## Integration

Never removes Failed/Rejected steps — StepOutcome variants confirmed real in crates/core/src/step.rs. bottlenecks()/rejected_branches() continue working post-compaction since the default policy never touches what they care about.

## Verification notes

Confirmed against real StepOutcome enum shape.

## Dependencies

- trace-archive

## Notes

Compaction is a one-way, lossy transform — decide the reversibility question (compact only a copy vs. accept the one-way door) before writing code.

## Prior art
No dedicated research agent was run for this one. Retention-policy-driven log/event compaction
(keep recent + keep anomalies, summarize the rest) is standard practice in observability tooling
(e.g. log-level sampling, span sampling in tracing backends) — not something requiring academic
grounding. The one open design question genuinely worth external input — is compaction reversible
— is answered the same way most observability systems answer it (no, sampling/compaction is a
one-way door in production tracing systems generally), which supports this doc's existing framing
without needing new citations.
