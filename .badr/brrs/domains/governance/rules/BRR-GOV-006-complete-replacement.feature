---
domain: governance
effective_from: "2026-11-10T12:45:55.982162000Z"
id: BRR-GOV-006
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-006
@domain:governance
Feature: Replacement requires complete declared groups
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/replacement.rs; tests/relationships.rs
  Rule: Replacement requires complete declared groups
    Scenario: Supported outcome
      Given eligible same-type predecessors and qualified successors
      When an explicitly authorized complete replacement is performed
      Then reciprocal replacement links and lifecycle changes are recorded
    Scenario: Boundary behavior
      Given a declared replacement omits part of a required group or creates a cycle
      When replacement is requested
      Then the operation is refused before publication
