---
domain: architecture
effective_from: "2026-11-10T12:44:33.926253000Z"
id: BRR-ARC-002
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-002 @domain:architecture
Feature: Design requires a valid repository
  Rule: Design proceeds only against a validated, unlocked repository
    Scenario: Valid repository
      Given initialized configuration, a resolvable local baseline and no .badr.lock
      When validation reports complete required coverage
      Then design may read records and save reviewed ADRs
    Scenario: Invalid, incomplete or locked state
      Given validation fails, coverage is incomplete, or a lock exists
      When design is requested
      Then saving is blocked, the cause is reported, and any interview is explicitly labelled limited
