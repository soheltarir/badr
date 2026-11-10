---
domain: governance
effective_from: "2026-11-10T12:45:34.938943000Z"
id: BRR-GOV-003
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-003
@domain:governance
Feature: Approval and business applicability are separate
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/authoring.rs; src/replacement.rs; tests/evidence.rs
  Rule: Approval and business applicability are separate
    Scenario: Supported outcome
      Given a structurally valid proposed business rule
      When an explicitly authorized reviewed approval is recorded
      Then the rule becomes approved but remains inactive until explicit activation
    Scenario: Boundary behavior
      Given a proposed architecture decision
      When an explicitly authorized reviewed approval is recorded
      Then the decision becomes approved and directs new work
