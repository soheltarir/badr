---
domain: governance
effective_from: "2026-11-10T12:45:28.184897000Z"
id: BRR-GOV-002
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-002
@domain:governance
Feature: Allocated identities remain reserved
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/authoring.rs; tests/authoring.rs
  Rule: Allocated identities remain reserved
    Scenario: Supported outcome
      Given a registered domain and retained allocation evidence
      When a new business rule is created
      Then a unique domain-prefixed identity is allocated and registered as draft
    Scenario: Boundary behavior
      Given an identity appears in retained history but its file was deleted
      When another record is allocated
      Then the previous identity is not reused
