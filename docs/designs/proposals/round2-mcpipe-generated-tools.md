---
title: mcpipe-generated tools
slug: mcpipe-generated-tools
round: 2
status: draft
viability: medium-high
depends_on:
- capability-typed-tools
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# mcpipe-generated tools

## Problem

Every tool declaration has to be hand-written; mcpipe already solves the identical translation problem for shell CLIs.

## Approach

Extend mcpipe with a new generation target (`--target trace-agent`) emitting `Tool` impls from an OpenAPI/GraphQL/MCP spec.

## API sketch

CLI: `mcpipe generate --target trace-agent --spec ./openapi.yaml --out ./src/tools.rs`; generated code implements a `Tool` trait (not yet defined — see capability-typed-tools) with `type Input`/`type Output` and `async fn call(&self, input: Self::Input) -> Result<Self::Output, TraceErr>`.

## Integration

Additive to mcpipe as a new --target, not a new tracers crate.

## Verification notes

Confirmed mcpipe is real with working openapi_gen.rs and backend/openapi.rs (~/dev/mcpipe/src), and a documented --gen-openapi flag family. But no --target trace-agent exists today, and no Tool trait exists in tracers-agent to target.

## Dependencies

- capability-typed-tools

## Notes

The 'mostly plumbing' framing in the original proposal is optimistic — real glue work mapping mcpipe's CLI-oriented errors to TraceErr, and it's blocked on a Tool trait existing first (capability-typed-tools).

## Prior art
No dedicated research agent was run for this one. Code generation from an API spec (OpenAPI/GraphQL
-> typed client bindings) is a mature, widely-practiced pattern (openapi-generator, graphql-codegen,
progenitor — the last of which mcpipe itself already depends on per its Cargo.toml) with no research
question left open; the genuinely open item is the internal error-mapping design between mcpipe's
CLI-oriented errors and TraceErr, which is project-specific, not literature-answerable.
