---
domain: governance
effective_from: "2026-11-10T12:46:34.029591000Z"
id: BRR-GOV-011
owners:
- BADR maintainers
related_architecture_decisions: []
status: effective
superseded_by: []
supersedes: []
---
@brr:BRR-GOV-011 @domain:governance
Feature: Authorizing actor defaults to the git identity
  Rule: An omitted actor defaults visibly to the git user name
    Scenario: Actor omitted
      Given git user.name is set and a governed command omits --actor
      When the command runs with confirmation and a rationale
      Then the git user name is recorded as actor and the event is marked as defaulted
    Scenario: No identity
      Given git user.name is unset and the actor is omitted
      When the command runs
      Then it is refused before publication
    Scenario: Explicit actor
      Given --actor is supplied
      When the command runs
      Then the supplied actor is recorded and not marked as defaulted
