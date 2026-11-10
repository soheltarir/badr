---
domain: governance
id: BRR-GOV-007
owners:
- BADR maintainers
status: draft
---
@brr:BRR-GOV-007
@domain:governance
Feature: Ordinary related links are directed
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/relationships.rs; tests/relationships.rs
  Rule: Ordinary related links are directed
    Scenario: Supported outcome
      Given two existing records
      When an authorized ordinary related link is added
      Then only the source outgoing relationship is changed
    Scenario: Boundary behavior
      Given a cycle of ordinary related links
      When outgoing relationships are traversed
      Then each reachable record is visited once
