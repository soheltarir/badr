---
domain: architecture
effective_from: "2026-11-10T12:45:02.598249000Z"
id: BRR-ARC-007
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-007 @domain:architecture
Feature: Offer to bring rules to effective
  Rule: A non-effective BRR reaches effective only through authorized steps
    Scenario: Offer
      Given a needed BRR is draft, proposed or approved
      When it is discovered during design
      Then the user is offered proposal, approval and activation as separate authorized steps
    Scenario: Declined
      Given the user declines
      When design continues
      Then the decisions proceed as not BRR-backed
    Scenario: Authorization
      Given no explicit authorization was given for a step
      When that step would run
      Then it does not run
