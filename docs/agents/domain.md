# Domain docs

This is a single-context repository.

## Before exploring

- Read `docs/specs/badr-spec.md` for BADR’s confirmed requirements.
- Read the root `GLOSSARY.md` when present.
- Read relevant repository architecture decisions under `docs/adr/`
  when present.

If the glossary or ADR directory is absent, proceed silently. Create
domain documentation lazily when terminology or decisions are resolved.

Use glossary terms consistently in tickets, code, tests, and proposals.
Flag conflicts with existing ADRs explicitly rather than overriding them
silently. Note unresolved terminology for domain-modeling.

## Boundaries

These paths document BADR’s own development. The managed `.badr/`
layout described by the specification is BADR’s product storage contract;
setup does not initialize it.

The confirmed BADR specification controls where sibling kata references
differ. Those references are provenance, not runtime dependencies.
