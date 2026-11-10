---
domain: architecture
effective_from: "2026-11-10T12:44:28.636850000Z"
id: BRR-ARC-001
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-001 @domain:architecture
Feature: Design inputs come from effective rules
  Rule: Architecture design starts from effective business rules or stated requirements
    Scenario: Effective BRRs supply the design inputs
      Given the user names effective BRRs or a domain containing effective BRRs
      When architecture design begins
      Then only those effective BRRs are treated as drivers
    Scenario: Non-effective BRRs are not drivers
      Given a named BRR is draft, proposed, approved or withdrawn
      When architecture design begins
      Then it is reported as not effective and is not treated as a driver
    Scenario: Free-form requirements are labelled
      Given the user supplies requirements instead of BRRs
      When a decision is produced
      Then it is labelled as not BRR-backed
