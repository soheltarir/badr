---
domain: architecture
effective_from: "2026-11-10T12:45:08.762310000Z"
id: BRR-ARC-008
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-008 @domain:architecture
Feature: Design ends at proposed ADRs
  Rule: The design workflow makes no implicit lifecycle change
    Scenario: After saving
      Given proposed ADRs were saved
      When the workflow completes
      Then no ADR is approved, rejected, deprecated or superseded
    Scenario: Git
      Given records were written
      When the workflow completes
      Then nothing is staged or committed and pending durability is reported
