---
id: "0005"
status: proposed
date: "2026-11-05"
---
# Explicit noninteractive agent interface

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Humans and agents need predictable commands and machine-readable outcomes.

## Decision Drivers

No implicit authorization; deterministic inspection; distinguish incomplete checks from failures.

## Considered Options

Explicit CLI arguments and versioned JSON; interactive-only workflows; inferred lifecycle changes.

## Decision Outcome

Use explicit content and authorization arguments with schema-versioned JSON and distinct exit codes. Only an explicitly selected editor may launch. BADR does not fetch, stage, or commit automatically.

### Consequences

Callers must inspect coverage and partial-write fields and arrange review and Git commits themselves.

## Implementation evidence

src/main.rs; docs/json-schema-v1.json; tests/cli.rs
