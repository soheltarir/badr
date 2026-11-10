---
domain: governance
id: BRR-GOV-005
owners:
- BADR maintainers
status: draft
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
