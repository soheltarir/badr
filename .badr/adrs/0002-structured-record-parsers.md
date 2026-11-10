---
id: "0002"
status: proposed
date: "2026-11-05"
---
# Established parsers and distinct record formats

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Business behavior and architecture rationale need different reviewable structures.

## Decision Drivers

Executable-style examples; explicit decision rationale; strict metadata validation.

## Considered Options

Gherkin BRRs and MADR-style ADRs with established parsers; a custom uniform text grammar.

## Decision Outcome

Parse BRR bodies with gherkin and ADR sections with CommonMark tooling, alongside strict YAML metadata checks. This preserves established syntax while applying BADR-specific structural contracts.

### Consequences

Multiple parsing dependencies are required; structural validity cannot establish semantic adequacy.

## Implementation evidence

src/brr.rs; src/adr.rs; Cargo.toml; tests/adr.rs
