# Trace decisions to BRRs and surface coverage gaps

Used in `badr-design-arch` after the handoff and again before saving. Nothing
here writes a file. Coverage is judged from **current records only**: never from
memory, an earlier listing, or the conversation.

## Coverage review

1. Re-read the records now: `badr --repo <repo> list --type brr --json` for the
   effective BRRs in scope (the chosen domain, or the named BRRs), and
   `badr --repo <repo> list --type adr --json`, then `show <id> --json` for each
   ADR, to read its `related_business_rules`.
2. An effective BRR is **addressed** when at least one **proposed or approved**
   ADR lists it in `related_business_rules`. Rejected, deprecated, and
   superseded ADRs do not address it. Only effective BRRs are judged; pending
   and history BRRs are not gaps.
3. Show a table: BRR ID, title, and each addressing ADR with its **status**
   (`approved` or `proposed`). A BRR covered only by a *proposed* ADR is
   covered but not settled; say so.
4. List every effective BRR with no addressing ADR as a **gap**. Offer an ADR for
   each, one question per gap: write one now (it enters decision selection as a
   candidate, see [Decision selection and ADR conflicts](decisions.md)), or
   leave it. A declined gap produces nothing and is not raised again this session.

Re-run the review after saving, because the saved ADRs change coverage.

## Decisions without a driver

A selected decision has a driver when it answers at least one **effective** BRR.
Before the exact-content confirmation in [Save accepted
decisions](saving.md), check each decision:

- If it has none (only **not BRR-backed** free-form drivers), warn that the ADR
  will carry no `related_business_rules` and is not traceable to any BRR.
- Require the user to explicitly confirm that decision. Saving is **not
  blocked**: with confirmation, the ADR is saved with an empty
  `related_business_rules`, and its prose keeps the **not BRR-backed** label.
- Decline means that decision is dropped, nothing else changes.

Drivers are declared when the ADR is created (see saving); they cannot be added
afterwards, so raise the warning before creation, not after.
