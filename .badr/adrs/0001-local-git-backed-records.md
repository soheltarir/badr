---
id: "0001"
status: proposed
date: "2026-11-05"
---
# Local Git-backed records

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Records need portable review and a durable audit trail without a hosted service.

## Decision Drivers

Local operation; readable diffs; no runtime kata dependency.

## Considered Options

Fixed Markdown and Gherkin files with Git history; hosted database or external workflow service.

## Decision Outcome

Use the fixed .badr layout and an explicitly configured local baseline. Git retains committed history while BADR records local evidence. This keeps ordinary operations offline and repository-owned.

### Consequences

Uncommitted changes lack Git durability; shallow or unavailable history limits validation.

## Implementation evidence

src/repository.rs; docs/evidence.md; tests/runtime.rs
