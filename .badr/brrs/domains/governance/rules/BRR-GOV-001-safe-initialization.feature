---
domain: governance
effective_from: "2026-11-10T12:45:21.545352000Z"
id: BRR-GOV-001
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-001
@domain:governance
Feature: Initialization preserves repository data
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/repository.rs; tests/cli.rs
  Rule: Initialization preserves repository data
    Scenario: Supported outcome
      Given an existing Git repository with an explicitly selected local baseline
      When initialization is repeated with matching configuration
      Then the configuration bytes are preserved and unrelated files remain unchanged
    Scenario: Boundary behavior
      Given a conflicting configuration already exists
      When initialization is requested
      Then the conflict is reported without overwriting configuration
