---
title: 'trace:: as a compilation target for .crux pipelines'
slug: crux-compilation-target
round: 4
status: draft
viability: medium
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# trace:: as a compilation target for .crux pipelines

## Problem

planning-with-crux's pipeline DSL (pipe/join_all/speculate/route_on_confidence/delegate) and tracers-runtime's combinators are the same five ideas built twice in two systems that don't know about each other.

## Approach

New crux-trace crate or --target trace mode compiling .crux YAML into calls against tracers-agent/tracers-runtime instead of (or alongside) crux's own runtime.

## API sketch

Compiler emits calls like `trace_runtime::speculate(candidates, task).await` and `trace_agent::delegate(&HumanReviewer, task, &ctx).await` from .crux combinators.

## Integration

Every .crux combinator maps onto something that already shipped: join_all -> trace_runtime::join_all, speculate -> trace_runtime::speculate, delegate -> trace_agent::delegate.

## Verification notes

Confirmed crux is real (~/dev/crux), with a real Crux<T> struct (crux-types/src/crux_value.rs, crux-runtime/src/ctx.rs) and real .crux YAML pipeline support per its README. Did not verify crux-runtime actually exposes named pipe/speculate/route_on_confidence functions matching tracers-runtime's combinators one-to-one — worth grepping crux-runtime before committing.

## Notes

Most ambitious proposal in round four. Worth a real design spike before committing — .crux's looser YAML-level abstraction and trace::'s strict Rust typing may not actually want to be the same system.

## Prior art
No dedicated research agent was run for this one — this is entirely an internal-integration
question between two local, already-owned projects (trace:: and crux), not something external
literature addresses. Compiling one DSL to another runtime's primitives (a "compilation target")
is a standard compiler-construction pattern generally, but that generic fact doesn't help decide
whether crux's YAML-level abstraction and trace::'s Rust-typed combinators actually want to be the
same system — that's a judgment call specific to these two sibling projects, resolvable only by
reading crux-runtime's actual source (not yet done — see this doc's Verification notes) and by
discussion with the person who owns both repos.
