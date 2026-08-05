---
title: contract! sourced from course-correct-rules.json
slug: contract-from-coursers-rules
round: 6
status: draft
viability: medium
depends_on:
- contract-macro
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# contract! sourced from course-correct-rules.json

## Problem

coursers' rules file already encodes exactly the kind of postcondition contract! is designed to check, but as regex rules consumed by a separate hook process invisible to the agent's own reasoning.

## Approach

A Contract::pre() checker loading and evaluating course-correct-rules.json directly, giving the agent a structured TraceErr instead of an opaque external denial.

## API sketch

Worked instance of contracts_core::func::Contract, checker loading `CourseCorrectRules::load_default()`.

## Integration

Third worked instance of contract! across rounds four through six, following the same fixed-mechanism/swappable-checker shape.

## Verification notes

Confirmed course-correct-rules.json's real schema matches exactly (see coursers-step-shape and failure-learning-decay-reference for the same verified schema).

## Dependencies

- contract-macro

## Notes

Unlike the rustqual and verification-before-completion instances, coursers pre is a live, already-running enforcement mechanism — this would duplicate rule evaluation rather than fill a gap. Resolve the doc's own open question first: is duplicated evaluation worth it purely for causal_chain() visibility, or should this replace coursers' hook for trace::-native agents specifically?

## Prior art
No dedicated research agent was run for this one — worked instance of contract! against an
already-verified local rules file (course-correct-rules.json). The open question this doc already
raises (is duplicating coursers' live enforcement inside agent reasoning worth it purely for
causal_chain() visibility) is a local cost/benefit judgment call, not a research question.
