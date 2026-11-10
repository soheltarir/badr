---
id: "0003"
status: proposed
date: "2026-11-05"
---
# Evidence-backed lifecycle and exact body digests

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Editable status fields alone cannot explain authorization or protect approved reasoning.

## Decision Drivers

Explicit transitions; frozen approval content; honest trust boundaries.

## Considered Options

Replayable local receipts and SHA-256 body digests; status-only metadata; authenticated remote ledger.

## Decision Outcome

Retain operation evidence and check exact approved body digests against available Git revisions. Separate BRR approval from activation; approved ADRs direct new work. This makes local lifecycle changes inspectable without claiming authentication.

### Consequences

Coordinated rewriting of records evidence and history is outside the trusted-local model. Eligible branch-local resets require fresh approval.

## Implementation evidence

src/evidence.rs; src/authoring.rs; src/editing.rs; docs/evidence.md
