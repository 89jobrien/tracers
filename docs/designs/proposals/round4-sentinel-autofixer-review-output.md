---
title: sentinel-autofixer-compatible review output
slug: sentinel-autofixer-review-output
round: 4
status: draft
viability: low
depends_on:
- trace-narrate
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# sentinel-autofixer-compatible review output

## Problem

A trace:: code-review agent's Trace<Review> isn't in a format sentinel-autofixer (which already applies suggestion-level fixes from sentinel reports) can consume.

## Approach

`Trace::as_sentinel_report()` projects accepted Branches into sentinel's expected finding shape.

## API sketch

`struct SentinelCompatibleFinding { file: String, line: usize, suggestion: String, severity: SentinelSeverity }`; `impl<T> Trace<T> { fn as_sentinel_report(&self) -> Vec<SentinelCompatibleFinding> }`

## Integration

Pure projection, same shape as trace-narrate or view-as-audience — no new data captured.

## Verification notes

No sentinel-autofixer binary or source directory found anywhere on this machine, despite direct search.

## Dependencies

- trace-narrate

## Notes

Cannot verify this tool exists in this environment. Confirm with the user before scoping — 'sentinel' as a godmode agent/skill exists (structured code reviewer), but 'sentinel-autofixer' specifically as a distinct consuming tool was not found.

## Prior art
No research was performed — the target tool (sentinel-autofixer specifically, as distinct from
the real "sentinel" code-review agent) could not be located anywhere on this machine during
verification (see Verification notes). Nothing to research until its existence is confirmed.
