---
title: whatidid-compatible trace event stream
slug: whatidid-event-stream
round: 4
status: draft
viability: low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# whatidid-compatible trace event stream

## Problem

whatidid only harvests Claude Code chat session JSONL — autonomous trace:: agent runs with no human in the loop are invisible to it.

## Approach

`Trace::as_activity_events()` renders a trace as whatidid-compatible JSONL lines, written alongside TaskRegistry's normal checkpoint.

## API sketch

`struct TraceActivityEvent { kind: &'static str, agent_name: String, goal: String, outcome: String, timestamp: DateTime<Utc> }`; `impl<T> Trace<T> { fn as_activity_events(&self) -> Vec<TraceActivityEvent> }`

## Integration

Writes into the same ~/.claude/projects/*/*.jsonl-shaped location whatidid already scans — no new harvesting code needed on whatidid's side.

## Verification notes

No whatidid binary or source directory found anywhere on this machine, despite a direct search.

## Notes

Cannot verify this tool exists in this environment. Confirm with the user before scoping further — may be confused with the godmode agent-improvement-loop's 'collect traces' stage (round five, proposal 7), which is a verified, real target.

## Prior art
No research was performed — the target tool (whatidid) could not be located anywhere on this
machine during verification (see Verification notes). Same situation as gkg-linked-tracegraph:
nothing to research until the tool's existence is confirmed with the user.
