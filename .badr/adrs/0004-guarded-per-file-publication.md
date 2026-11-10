---
id: "0004"
status: proposed
date: "2026-11-05"
---
# Guarded per-file publication

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Governed operations can modify several files while concurrent writers or filesystem failures threaten consistency.

## Decision Drivers

Preserve unrelated data; refuse unsafe paths; report partial writes accurately.

## Considered Options

Writer lock and atomic individual file publication; transaction journal with recovery; unchecked in-place writes.

## Decision Outcome

Use a repository-local writer lock, complete pre/postvalidation, baseline and content checks, and descriptor-relative publication. Retain a repair lock after publication failures. This implements bounded local protection without claiming multi-file atomicity.

### Consequences

Crashes may leave partial changes. Recovery is manual; arbitrary editors do not honor the lock and final rename races remain a limitation.

## Implementation evidence

src/mutation.rs; docs/cli.md; tests/domain.rs
