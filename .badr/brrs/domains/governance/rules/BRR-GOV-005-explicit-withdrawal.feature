---
domain: governance
effective_from: "2026-11-10T12:45:48.823318000Z"
id: BRR-GOV-005
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-005
@domain:governance
Feature: Withdrawal does not revive previous policy
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/authoring.rs; src/replacement.rs; tests/relationships.rs
  Rule: Withdrawal does not revive previous policy
    Scenario: Supported outcome
      Given an effective business rule
      When explicitly authorized retirement is requested
      Then the applicability interval ends and the resulting policy gap is exposed
    Scenario: Boundary behavior
      Given a replacement successor that superseded a predecessor
      When the successor is withdrawn
      Then the predecessor remains superseded
