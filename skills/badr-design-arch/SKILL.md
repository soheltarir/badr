---
name: badr-design-arch
description: Start an architecture design session from effective BADR BRRs, user-named BRRs, or free-form requirements on a validated repository, then select decisions with ADR-conflict handling. Use when designing architecture decisions; writes nothing.
---

# Start an architecture design session with BADR

Establish the drivers and the validity of repository state, then hand off to the
design interview. **This skill writes nothing**: no BRRs, ADRs, glossaries,
memory, or temporary files in the repository. Requires Git and a working BADR
CLI; memory is optional.

## 1. Set the source boundary

Apply the source-boundary rules in [Preflight and sources](references/preflight.md):
canonical Git root, memory namespace `badr.source-preferences.v1`, validation of
recovered preferences, reconfirmation, and current-checkout bytes. Store nothing
except a user-confirmed source preference under that namespace/key.

## 2. Preflight and validate

Follow [Preflight and sources](references/preflight.md): CLI version/help,
`.badr/config.yaml`, exact local `refs/heads/<baseline_branch>`, root
`.badr.lock`, then:

```sh
badr --repo <repo> validate --json
```

Exit handling decides the **session mode**:

| Exit | Meaning | Mode |
| --- | --- | --- |
| 0, complete required coverage | valid | **Full**: BRR inspection permitted |
| 2 | invalid state/integrity | **Blocked for saving**; interview only if the user chooses it, labelled **limited** |
| 3 | incomplete coverage | **Blocked for saving**; remediate, or the user chooses a **limited** interview |
| 4, or `.badr.lock` present | operational/lock failure | **Blocked**; preserve and report the lock, never remove it |
| missing/invalid setup | no config or baseline | **Blocked**; no BADR inspection |

A blocked or limited session must say so explicitly at the start of the
handoff, with the diagnostics and the coverage that is missing. Invalid,
incomplete, or locked state never permits saving. This skill saves nothing in
any mode; the label governs what later steps may do.

## 3. Choose the drivers

Ask which input the user is providing, or infer it only when unambiguous:

1. **A domain**: `badr --repo <repo> list --type brr --json`, then take the
   domain's effective BRRs.
2. **Named BRRs**: `badr --repo <repo> show <id> --json` for each ID named.
3. **Free-form requirements**: text from the user.

Inspection (1 and 2) requires Full mode. In a limited session, read BRR files
directly only if the user chose that, and mark every finding unvalidated.

### Only effective BRRs drive

Classify every candidate by status. Only **effective** BRRs are drivers. Report
each non-effective BRR separately with ID, status, and path: drafts, proposals,
and approvals are *pending*; withdrawn rules are *history*; replaced rules name
their successor when known. Never promote them to drivers; ask whether to
continue without them. Approval is not activation.

If a named ID does not exist, say so; do not guess a neighbour.

### Free-form requirements

Accept them as given. Every decision resting on them, and every resulting item
in the handoff, is labelled **not BRR-backed**. Do not convert them into BRRs or
imply they are effective rules. If they conflict with an effective BRR, surface
the conflict with the BRR's ID and ask which intent wins.

Mixed input is allowed: effective BRRs keep their IDs; free-form items keep the
**not BRR-backed** label.

## 4. Gather context (read only)

In Full mode, read the driver BRRs fully, `.badr/brrs/registry.yaml`, affected
glossaries under `.badr/brrs/domains/<domain>/glossary.md`, and related ADRs in
`.badr/`, with statuses and relationships. Approved ADRs and effective BRRs
outrank extra documents. Surface conflicts and gaps with source path and status.

## 5. Hand off to the interview

End with a handoff, not a save:

- session mode: **Full** or **limited** (with reason and missing coverage);
- source preference and boundary in force;
- effective drivers (ID, domain, title) and, separately, non-effective BRRs
  reported as such;
- free-form requirements, each marked **not BRR-backed**;
- conflicts, gaps, and related ADRs;
- the first frontier of design questions, each with a recommendation.

Then begin the design interview with decision selection (step 6). Recording
ADRs requires a separate request. No staging, commits, or publishing.

## 6. Select and decide

Follow [Decision selection and ADR conflicts](references/decisions.md). Only
user-selected decisions proceed, and unselected ones never produce an ADR. This
step still writes nothing.
