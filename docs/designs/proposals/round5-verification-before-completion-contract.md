---
title: contract! gated on godmode:verification-before-completion
slug: verification-before-completion-contract
round: 5
status: draft
viability: medium
depends_on:
- contract-macro
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# contract! gated on godmode:verification-before-completion

## Problem

verification-before-completion's checklist is something an agent has to remember to consult; nothing structurally prevents emitting Trace<Done> without having satisfied it.

## Approach

A Contract::post() sourcing the actual checklist from the skill file itself, so a 'done' claim fails structurally if unsatisfied.

## API sketch

Worked instance of contracts_core::func::Contract, checker loading `godmode_skill::load_checklist("verification-before-completion")`.

## Integration

Same shape as rustqual-gated-contracts — contract!'s mechanism stays fixed, only the checker changes.

## Verification notes

Confirmed godmode:verification-before-completion exists as a real skill.

## Dependencies

- contract-macro

## Notes

Skill files are prose, written for an LLM to read, not structured data a Contract can mechanically check — this requires either rewriting the checklist into a structured checkable format (risking readability for other consumers) or a looser LLM-mediated check that isn't a true compile/runtime guarantee. Resolve this before committing to the design.

## Prior art
No dedicated research agent was run for this one — this is a worked instance of contract! against
a local, already-verified godmode skill. The real open question this doc already identifies
(skill files are prose written for an LLM to read, not structured data a Contract can mechanically
check) is a concrete engineering problem specific to this codebase's own skill-file format, not
something general research on prose-to-structured-data conversion would meaningfully narrow.
