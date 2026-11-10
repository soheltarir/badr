---
id: "0006"
status: proposed
date: "2026-11-05"
---
# Binary and checksum-verifying curl distribution

Retrospective documentation of implementation at `11a64e2`; proposed for human review, not a reconstructed historical approval.

## Context and Problem Statement

Users need installation without a Rust development environment.

## Decision Drivers

Small local runtime; verified downloads; supported macOS and Linux targets.

## Considered Options

Versioned binary archives and curl installer; package-manager tap; mandatory source build.

## Decision Outcome

Distribute versioned archives and SHA-256 checksums with install.sh as the supported end-user path. Verify checksum and executable version before user-local installation. The current repository removed Homebrew distribution.

### Consequences

Release engineering must verify target builds. Ordinary runtime needs trusted Git; installation needs network access. This seed does not independently certify every platform.

## Implementation evidence

install.sh; scripts/prepare-release.sh; README.md; commit 39973c9
