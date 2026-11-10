---
domain: governance
id: BRR-GOV-008
owners:
- BADR maintainers
status: draft
---
@brr:BRR-GOV-008
@domain:governance
Feature: Validation reports its coverage without changing records
  # Retrospective seed at 11a64e2; pending human review.
  # Evidence: src/repository.rs; src/evidence.rs; tests/evidence.rs
  Rule: Validation reports its coverage without changing records
    Scenario: Supported outcome
      Given a valid managed collection with complete available history
      When read-only validation is requested
      Then configuration collection evidence and history coverage are reported separately
    Scenario: Boundary behavior
      Given required Git history is unavailable
      When validation is requested
      Then incomplete coverage is reported rather than a complete success
