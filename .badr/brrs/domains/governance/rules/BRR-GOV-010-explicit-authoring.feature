---
domain: governance
id: BRR-GOV-010
owners:
- BADR maintainers
status: draft
---
@brr:BRR-GOV-010
@domain:governance
Feature: Authoring grants no implicit approval
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/authoring.rs; src/editing.rs; tests/authoring.rs; tests/editing.rs
  Rule: Authoring grants no implicit approval
    Scenario: Supported outcome
      Given a registered domain
      When a valid business rule is created and explicitly proposed
      Then the record progresses from draft to proposed without becoming effective
    Scenario: Boundary behavior
      Given a proposed architecture decision
      When its ordinary content is edited
      Then it remains proposed without an approval receipt
