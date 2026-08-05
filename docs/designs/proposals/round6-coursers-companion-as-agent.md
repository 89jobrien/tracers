---
title: coursers-companion.md as a real trace_agent::Agent
slug: coursers-companion-as-agent
round: 6
status: draft
viability: medium-low
depends_on: []
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# coursers-companion.md as a real trace_agent::Agent

## Problem

coursers-companion's diagnostic reasoning about why a command kept failing lives entirely in a chat transcript, not a structured, inspectable Trace<Diagnosis>.

## Approach

Reimplement coursers-companion as a typed Agent whose Input is coursers' own FailureReport shape and whose considered explanations become Branch entries.

## API sketch

`struct CoursersCompanion; impl Agent for CoursersCompanion { type Input = FailureReport; type Output = Diagnosis; ... }`

## Integration

FailureReport is data coursers already produces (the rolling failure log's entries) — no new data capture required, only a new consumer.

## Verification notes

Confirmed agents/coursers-companion.md exists as a real file. Did not read its actual prose content to assess whether its diagnostic reasoning is structured enough to type as a clean Diagnosis output.

## Notes

Read coursers-companion.md's actual content before scoping further — same prose-vs-structured-data gap flagged for verification-before-completion-contract. May need Diagnosis to stay loose (String plus optional structured fields) to preserve free-form reasoning flexibility.

## Prior art
No dedicated research agent was run for this one. The file (agents/coursers-companion.md) was
confirmed to exist but its actual prose content was not read (see Verification notes) — that's
the real blocker to scoping this further, not a gap external research could fill. Whether an
existing prose-based agent definition can be typed as Agent without losing reasoning flexibility
is answered by reading that specific file, not by literature on agent architectures generally.
