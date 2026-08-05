---
title: gkg-linked TraceGraph
slug: gkg-linked-tracegraph
round: 4
status: draft
viability: low
depends_on:
- trace-graph
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# gkg-linked TraceGraph

## Problem

TraceGraph tracks trace-to-trace lineage; gkg tracks codebase symbol relationships. A Coder agent's trace has no idea gkg already knows the code structure it touched, and vice versa.

## Approach

`AgentContext::touch_symbol(symbol)` looks up gkg's index instead of storing a bare string, producing a GkgSymbolRef-backed Step.

## API sketch

`struct GkgLinkedStep { step: Step, symbols_touched: Vec<GkgSymbolRef> }`; `impl AgentContext { fn touch_symbol(&mut self, symbol: &str) -> Result<GkgSymbolRef, TraceErr> }`

## Integration

TraceGraph::downstream_of() and gkg context become jointly answerable.

## Verification notes

No gkg binary or source tree found anywhere on this machine, despite an extensive search. Cannot verify this tool exists in this environment at all.

## Dependencies

- trace-graph

## Notes

Unverifiable as scoped — either gkg is a tool from a different environment/machine, deprecated, or aspirational. Do not schedule until gkg's existence and API are confirmed directly with the user.

## Prior art
No research was performed — the target tool (gkg) could not be located anywhere on this machine
during verification (see Verification notes). There is nothing to research until gkg's existence
and actual API are confirmed with the user; researching code-knowledge-graph literature in the
abstract wouldn't establish anything about this specific, unverified integration claim.
