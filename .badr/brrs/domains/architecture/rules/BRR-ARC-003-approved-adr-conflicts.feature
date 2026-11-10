---
domain: architecture
effective_from: "2026-11-10T12:44:39.362765000Z"
id: BRR-ARC-003
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-003 @domain:architecture
Feature: Conflicts with approved decisions need a human choice
  Rule: Overlap with an approved ADR is resolved by the user
    Scenario: Approved ADR overlaps a proposed decision
      Given an approved ADR covers the same decision
      When a new decision is being drafted
      Then the user chooses to keep the existing ADR, to write a successor, or to record a linked separate ADR
    Scenario: Proposed ADR overlaps
      Given only a proposed ADR covers the same decision
      When a new decision is being drafted
      Then the user is informed and drafting continues without a mandatory choice
