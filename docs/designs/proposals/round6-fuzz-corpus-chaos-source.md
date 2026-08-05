---
title: coursers' fuzz corpus as ChaosAgent's fault-injection source
slug: fuzz-corpus-chaos-source
round: 6
status: draft
viability: medium
depends_on:
- chaos-testing
source: docs/ideas/FEATURES.md and conversation rounds 2-6
---

# coursers' fuzz corpus as ChaosAgent's fault-injection source

## Problem

chaos-testing's ChaosPolicy sketch hand-specifies failure probabilities — essentially guessing at realistic failure modes worth injecting.

## Approach

`CorpusChaosPolicy` samples adversarial inputs from coursers' existing fuzz/ corpus instead of a synthetic probability distribution.

## API sketch

`struct CorpusChaosPolicy { corpus_path: PathBuf }`; `impl CorpusChaosPolicy { fn sample_adversarial_input(&self) -> BashCommand }`

## Integration

Directly extends ChaosAgent/ChaosPolicy with a real, curated corpus as an alternative or additional mode.

## Verification notes

Confirmed fuzz/ is real (~/dev/coursers/fuzz), with fuzz_targets/fuzz_rule_check.rs being the closest genuine match. CORRECTION: the other five fuzz targets (fuzz_pipe_stages, fuzz_ast_parse, fuzz_jsonl_parse, fuzz_pipeline, fuzz_expand) fuzz coursers' own internal parsing/pipeline logic, not a curated corpus of realistic adversarial Bash *commands* as the proposal implies — narrower than 'coursers' fuzz corpus' suggests.

## Dependencies

- chaos-testing

## Notes

Scope specifically to fuzz_rule_check's corpus, not 'coursers' fuzz corpus' generally, unless the other corpora are reviewed and found to contain reusable Bash-command-shaped inputs.

## Prior art
Shares its research grounding with chaos-testing (round 3) — see that doc's Prior art section,
which directly validates this proposal's core idea (chaos-engineering doctrine explicitly favors
real/representative failure sources over hand-specified probabilities). No additional research
was run specifically for this proposal beyond the correction already made in Verification notes:
only one of coursers' six fuzz targets (fuzz_rule_check) is confirmed to fuzz Bash-command-shaped
input specifically — the others fuzz unrelated internal parsing logic (AST, JSONL, pipeline
stages), so "coursers' fuzz corpus" is narrower in practice than the proposal's framing suggests.
