---
title: TaskRegistry as the backing store for GODMODE.tasks.yaml
slug: taskregistry-godmode-tasks
round: 5
status: draft
viability: high
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# TaskRegistry as the backing store for GODMODE.tasks.yaml

## Problem

godmode:task-management maintains its own bespoke YAML task graph; TaskRegistry independently does dependency-aware task tracking. A godmode session and a trace:: pipeline running side by side maintain two disconnected task graphs.

## Approach

GODMODE.tasks.yaml becomes a serialized TaskRegistry using the same Task/TaskStatus/Priority types already shipped, with godmode's skill commands becoming a thin CLI wrapper over the registry.

## API sketch

`TaskRegistry::load("GODMODE.tasks.yaml")`; `registry.ready_tasks().first()` for 'what's next' — no new API, reuse of the existing tracers-task surface.

## Integration

Confirmed tracers-task is done, tested, and already does everything godmode:task-management's own description asks for.

## Verification notes

Confirmed real via crates/task/src/lib.rs, registry.rs, task.rs reads — TaskRegistry, Task, TaskStatus, Priority all match the claimed capabilities exactly. Also confirmed godmode:task-management exists as a real skill.

## Notes

Strongest proposal across all rounds by direct comparison — this is deduplication of two already-built things, not new design. Task likely needs an open metadata: serde_json::Value field to absorb godmode-specific state (agent dispatch metadata, TDD phase tracking) without trace-task needing to know what it means.

## Prior art
No dedicated research agent was run for this one — this is deduplication between two local,
already-owned, already-verified systems (tracers-task's real TaskRegistry and godmode's real
task-management skill), not an open research question. The only genuinely open item (whether
Task needs an open metadata field to absorb godmode-specific state) is a local schema-design
decision.
