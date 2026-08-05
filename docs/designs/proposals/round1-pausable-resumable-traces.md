---
title: Pausable, resumable traces (human-in-the-loop)
slug: pausable-resumable-traces
round: 1
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# Pausable, resumable traces (human-in-the-loop)

## Problem

EscalationAction has no equivalent to Delegate for handing off to a human and genuinely stopping execution until an external decision arrives.

## Approach

New `EscalationAction::RequireApproval(ApprovalRequest)`; new `TaskStatus::Paused(ApprovalRequest)`; `resume(checkpoint, decision) -> TraceState<T>`.

## API sketch

`struct ApprovalRequest { question: String, context: serde_json::Value }`; `enum TraceState<T> { Complete(Trace<T>), Paused { checkpoint: PausedCheckpoint<T>, request: ApprovalRequest } }`; `struct PausedCheckpoint<T> { partial_trace: Trace<T>, agent_name: String, resume_input: serde_json::Value }`; `fn resume<T>(checkpoint: PausedCheckpoint<T>, decision: ApprovalDecision) -> TraceState<T>`

## Integration

TaskStatus (crates/task/src/task.rs) is a closed enum that already round-trips through TaskRegistry::save/load via serde — Paused is additive to a type designed to be extended this way. EscalationAction (crates/agent/src/hooks.rs) is also a closed enum built for exactly this kind of addition.

## Verification notes

Confirmed both TaskStatus and EscalationAction are real, closed, serde-friendly enums matching the doc's claims — checkpoint persistence (CheckpointStore) is real and store-agnostic.

## Notes

Real work is a resume() entrypoint and where PausedCheckpoint serialization lives — but no fighting the existing design.

## Prior art
This is a well-established workflow-orchestration pattern (durable execution / human-in-the-loop
approval gates), not a novel mechanism — production systems like Temporal.io and AWS Step
Functions have shipped "pause, serialize state, wait for external signal, resume" for years as
a core primitive. No dedicated research agent was run for this one; the pattern is standard
enough in the workflow-engine space that a literature search would mostly surface product docs,
not papers. The genuinely interesting design question here isn't "does this pattern work" (it
does, broadly) but the one this doc already identifies: how checkpoint staleness interacts with
confidence-decay when a pause lasts a long time — that's specific to this project, not something
external prior art resolves for us.
