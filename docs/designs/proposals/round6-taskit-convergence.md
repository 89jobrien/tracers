---
title: taskit convergence, alongside GODMODE.tasks.yaml
slug: taskit-convergence
round: 6
status: draft
viability: medium
depends_on:
- taskregistry-godmode-tasks
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# taskit convergence, alongside GODMODE.tasks.yaml

## Problem

Three independent task-tracking formats now exist across this toolchain (doob, GODMODE.tasks.yaml, taskit) — none aware of the others.

## Approach

Recognize the pattern directly: TaskRegistry's Task/TaskStatus/Priority types are general enough to be the single serialization all three converge on.

## API sketch

`TaskRegistry::load("taskit-protocol.lock")` — same types, different file, no new API beyond what taskregistry-godmode-tasks already proposes.

## Integration

Less a new feature than a call to resolve taskregistry-godmode-tasks and the doob trace-archive integration together, now with a third data point.

## Verification notes

Confirmed taskit.toml and taskit-protocol.lock are real files at ~/dev/coursers's repo root.

## Dependencies

- taskregistry-godmode-tasks

## Notes

Least actionable proposal on its own, most interesting as a signal. taskit-protocol.lock may encode a formal protocol-version contract (not just a schema) — read its actual content before assuming this is as simple as the GODMODE.tasks.yaml convergence. With three tools becoming TaskRegistry-backed, trace-task would need a real migration/versioning story since it'd be load-bearing for three separate external tools, not one experimental crate.

## Prior art
No dedicated research agent was run for this one — this is an internal-integration/deduplication
proposal across three local, already-owned systems (doob, GODMODE.tasks.yaml, taskit), not an
external research question. The open item (whether taskit-protocol.lock encodes a formal
protocol-version contract) is answered by reading that file's actual content, not by literature
search — see this doc's Notes, which already flags this as unread.
