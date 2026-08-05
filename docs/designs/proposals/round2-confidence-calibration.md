---
title: Confidence calibration
slug: confidence-calibration
round: 2
status: draft
viability: medium
depends_on:
- trace-archive
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Confidence calibration

## Problem

An agent's self-reported confidence is never checked against ground truth — two agents both reporting 0.9 could have very different real accuracy.

## Approach

`CalibrationTracker` records (predicted_confidence, was_correct) pairs per trace, buckets into confidence bins, reports predicted-vs-actual accuracy per bin.

## API sketch

`struct CalibrationRecord { trace_ref: TraceRef, predicted_confidence: f64, was_correct: bool, recorded_at: DateTime<Utc> }`; `struct CalibrationTracker { records: Vec<CalibrationRecord> }`; `impl CalibrationTracker { fn record(&mut self, trace_ref: TraceRef, predicted: f64, correct: bool); fn calibration_report(&self, agent_name: &str) -> CalibrationReport }`

## Integration

TaskStatus::Failed vs Done (confirmed real) is the most natural ground-truth source already in this repo. TraceArchive is the natural place to compute reports from.

## Verification notes

TaskStatus enum confirmed real and matches the claimed shape.

## Dependencies

- trace-archive

## Notes

Weaker standalone case until trace-archive exists to supply cross-session ground truth at scale — see doublecheck-confidence-adjustment (round 5) for a second, more immediate ground-truth source.

## Prior art

Shares its research grounding with confidence-decay (round 1) — see that doc's Prior art section for the full LLM-calibration literature (Kadavath 2022, Tian et al. 2023, and the 2025-2026 agentic-trajectory-calibration papers, especially arXiv:2601.15778's Holistic Trajectory Calibration, which calibrates over a whole trajectory rather than one output — directly analogous to bucketing by agent/trace here rather than per-output). No additional research was run specifically for this proposal; it's the same underlying research question (external ground truth needed because self-reported LLM confidence is unreliable) applied to a different data source (TaskStatus::Failed/Done instead of doublecheck's claim-verification signal).
