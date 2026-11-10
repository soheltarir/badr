---
domain: governance
id: BRR-GOV-009
owners:
- BADR maintainers
status: draft
---
@brr:BRR-GOV-009
@domain:governance
Feature: Governed writes preserve integrity and report partial outcomes
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/mutation.rs; tests/domain.rs
  Rule: Governed writes preserve integrity and report partial outcomes
    Scenario: Supported outcome
      Given a valid collection and an available repository writer lock
      When a governed mutation is requested
      Then the complete intended state is validated before publication and revalidated afterward
    Scenario: Boundary behavior
      Given a failure after publication begins
      When the mutation stops
      Then completed and pending writes are reported and a repair lock is retained
