---
domain: architecture
effective_from: "2026-11-10T12:44:44.951178000Z"
id: BRR-ARC-004
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-004 @domain:architecture
Feature: Users choose which decisions are written
  Rule: Every decision is user-selected with alternatives and a recommendation
    Scenario: Candidate decisions
      Given the drivers imply several independent architecture decisions
      When they are presented
      Then the user selects which decisions to write
    Scenario: Each selected decision
      Given the user selected a decision
      When its options are presented
      Then at least two real alternatives and a recommendation with reasons are shown before the user chooses
    Scenario: Unselected decisions
      Given the user did not select a decision
      When saving occurs
      Then no ADR is created for it
