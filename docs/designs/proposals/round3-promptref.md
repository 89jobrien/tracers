---
title: "PromptRef \u2014 prompt version provenance"
slug: promptref
round: 3
status: draft
viability: medium-low
depends_on:
- deterministic-replay
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# PromptRef — prompt version provenance

## Problem

No structured link from a Step back to exactly which version of a prompt produced it — correlating behavior change with prompt change is manual git-blame work.

## Approach

`PromptRef { template_name, version_hash, rendered_at }` attached per-step; version_hash is a hash of rendered content, not a manual counter.

## API sketch

`struct PromptRef { template_name: String, version_hash: String, rendered_at: DateTime<Utc> }`; `impl Step { fn with_prompt(mut self, prompt_ref: PromptRef) -> Self }`; `impl<T> Trace<T> { fn prompts_used(&self) -> Vec<&PromptRef> }`

## Integration

Pairs with replay()'s TraceDiff to attribute drift to prompt vs. sampling vs. input change.

## Verification notes

Confirmed Step has no prompt field today; with_prompt matches the existing with_confidence/with_duration builder pattern exactly, so the core-type addition itself is trivial.

## Dependencies

- deterministic-replay

## Notes

The 'integration' value proposition (automatic population via observe()) is blocked — observe is a language-design construct, not a real Rust type in this codebase yet. Viable today only as a manually-populated field.

## Prior art
No dedicated research agent was run for this one. Prompt versioning/hashing is common practice in
commercial MLOps tooling (e.g. prompt-management products that hash rendered templates for change
detection) but there's no rigorous academic literature specifically on this narrow mechanism worth
citing — it's a straightforward content-addressing idea, not a research question. The doc's own
correctly-identified blocker (no observe() construct exists yet to auto-populate this) is the real
gap, and no amount of external research resolves an internal infrastructure gap.
