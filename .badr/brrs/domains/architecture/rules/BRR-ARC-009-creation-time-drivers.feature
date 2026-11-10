---
domain: architecture
effective_from: "2026-11-10T12:45:15.081665000Z"
id: BRR-ARC-009
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-ARC-009 @domain:architecture
Feature: Drivers can be declared at ADR creation
  Rule: A new ADR may declare existing effective BRRs as drivers
    Scenario: Effective drivers
      Given the supplied ADR content lists existing effective BRR IDs
      When the ADR is created
      Then the relationships are recorded without a separate link step
    Scenario: Unsuitable drivers
      Given a listed BRR does not exist or is not effective
      When creation is requested
      Then it is refused before publication and no ID is reserved
    Scenario: Later changes
      Given a link must change after creation
      When the change is made
      Then it uses the separately authorized link operation
