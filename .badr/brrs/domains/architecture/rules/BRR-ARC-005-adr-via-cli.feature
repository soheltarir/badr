---
domain: architecture
effective_from: "2026-11-10T12:44:50.687625000Z"
id: BRR-ARC-005
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-005 @domain:architecture
Feature: Decisions are saved through the BADR CLI
  Rule: Each accepted decision becomes one proposed ADR after exact-content confirmation
    Scenario: Confirmed decision
      Given the user confirmed the exact ADR content and today's UTC date
      When it is saved
      Then one proposed ADR is created through the BADR CLI and validated afterwards
    Scenario: Unconfirmed or changed content
      Given content changed after confirmation
      When saving is requested
      Then renewed confirmation is required before any write
    Scenario: Interrupted save
      Given a write fails after publication began
      When the failure is reported
      Then completed and pending writes are reported and work stops without deletion or blind retry
