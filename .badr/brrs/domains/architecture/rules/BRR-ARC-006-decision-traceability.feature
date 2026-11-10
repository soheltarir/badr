---
domain: architecture
effective_from: "2026-11-10T12:44:56.566714000Z"
id: BRR-ARC-006
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-006 @domain:architecture
Feature: Decisions trace to their business rules
  Rule: Decisions record driving BRRs and expose coverage gaps
    Scenario: Driving BRRs
      Given an ADR is driven by effective BRRs
      When it is saved
      Then those BRRs are recorded as its drivers
    Scenario: Uncovered rule
      Given an effective BRR has no ADR addressing it
      When coverage is reviewed
      Then the gap is shown and the user is offered an ADR for it
    Scenario: Decision without a rule
      Given a decision has no driving BRR
      When it is presented for saving
      Then the user is warned and must confirm, and saving is not blocked
