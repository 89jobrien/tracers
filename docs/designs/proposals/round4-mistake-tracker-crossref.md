---
title: mistake-tracker cross-referencing
slug: mistake-tracker-crossref
round: 4
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# mistake-tracker cross-referencing

## Problem

A TraceErr or rejected Branch is exactly the kind of failure godmode's mistake-tracker exists to catalog, but the two systems don't share data at the moment of failure.

## Approach

`TraceErr::cross_reference()` checks the mistake-tracker's catalog synchronously at the moment an error is recorded.

## API sketch

`struct MistakeMatch { pattern_id: String, confidence: f64, prevention_note: Option<String> }`; `impl TraceErr { fn cross_reference(&self) -> Option<MistakeMatch> }`

## Integration

A Step that fails would carry Option<MistakeMatch> alongside StepOutcome::Failed.

## Verification notes

Confirmed godmode's mistake-tracker exists as a real skill/agent (godmode/skills/mistake-tracker, godmode/agents/mistake-tracker-agent.md) — this is the same underlying target as round five's proposals, just described as a standalone tool rather than correctly as a godmode skill.

## Notes

Requires the mistake-tracker's catalog to be queryable synchronously and cheaply during a live agent run — verify that's how it's structured today before assuming this integration is cheap.

## Prior art
No dedicated research agent was run for this one — this is an internal-integration proposal
between two local, already-owned tools (trace:: and the godmode mistake-tracker skill), not an
external research question. The open item that matters (whether the mistake-tracker's catalog is
structured for low-latency synchronous lookup during a live agent run) is answered by reading
that skill's actual implementation, not by literature search.
