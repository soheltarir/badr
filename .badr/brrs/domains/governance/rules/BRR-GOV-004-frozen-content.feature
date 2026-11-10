---
domain: governance
id: BRR-GOV-004
owners:
- BADR maintainers
status: draft
---
@brr:BRR-GOV-004
@domain:governance
Feature: Approved content cannot change silently
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/editing.rs; src/evidence.rs; tests/editing.rs
  Rule: Approved content cannot change silently
    Scenario: Supported outcome
      Given a record with an approval receipt
      When its frozen body bytes change outside an eligible authorized edit
      Then validation reports an integrity failure
    Scenario: Boundary behavior
      Given an approved record that has appeared in baseline history
      When an approval-reset edit is requested
      Then the reset is refused
