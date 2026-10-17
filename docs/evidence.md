# Evidence contract, version 1

`.badr/evidence.jsonl` is UTF-8 JSON Lines: one complete event per line, ending
in a newline. Empty/absent evidence is valid only for an empty record collection.
Duplicate JSON keys, unknown fields, blank/partial lines, unsupported versions,
malformed hashes/times/IDs, duplicate event IDs, and duplicate affected numeric
identities are errors. [The event schema](evidence-schema-v1.json) describes the
wire shape; replay adds state, relationship, and history constraints.

Domain registration creates governed registration events. Record approval,
external-edit registration, evidence repair, and lifecycle commands remain unavailable.
Missing evidence cannot be replaced by a successful syntax check. Do not invent
receipts to bless an existing status or changed body.

## Events and revisions

Every event has `version: 1`, a unique nonempty `event_id`, `operation`, `at`,
`branch`, `baseline_commit`, and a `changes` array (nonempty for record events). `at` has ISO 8601
seconds, a `T` separator, and UTC `Z`; times cannot be future or decrease within
the ledger. Fractional seconds and same-second operations are valid; time ordering
compares parsed instants, not timestamp spelling. `branch` is a literal Git branch
name. `baseline_commit` is a lowercase 40/64-digit locally available commit
in the configured baseline's history. Missing supporting history returns
incomplete coverage, never an invented historical result.

Governed events require nonempty `actor`, nonempty `rationale`, and
`confirmed: true`. Confirmation records the caller's declaration of human
review/authorization; it does not authenticate an approver or prove semantic
adequacy. Recorded times and branch names are declarations, not independently
authenticated observations. Allocation is not an approval.

Each change contains:

| Field | Meaning |
| --- | --- |
| `id`, `path` | Stable record ID and fixed-root repository-relative path. |
| `from`, `to` | Prior/resulting status; `from` is null/omitted for allocation. |
| `body_sha256` | Lowercase SHA-256 of the exact resulting decision/Gherkin body. |
| `approval_ref` | Active approval event ID; null/omitted before approval. An edit/reset references the prior receipt. |
| `related` | Directed related IDs (both ADR/BRR types for an ADR; ADR IDs for a BRR). |
| `supersedes`, `superseded_by` | Completed same-type replacement links. |
| `date`, `notes_sha256` | Required for ADR revisions: UTC event date and exact lifecycle-section digest. Forbidden for BRRs. |
| `effective_from`, `effective_until` | BRR applicability, absent before activation, retained after withdrawal. Forbidden for ADRs. |

Relationship arrays default to empty when omitted and cannot contain duplicate
numeric identities. ADR padding does not create a new identity. Paths cannot
escape `.badr/`, change an allocated identity's path, or traverse symlinks or
special files. Evidence-only fields never enter record frontmatter.

The following operations are understood:

| Operation | Permitted transition |
| --- | --- |
| `allocate` | New BRR → draft; new ADR → proposed. The event reserves its ID/path. |
| `propose` | BRR draft → proposed. |
| `decide` | Proposed → approved, referencing this event's own ID as `approval_ref`. |
| `edit` | Eligible approved revision → proposed with a changed body and the prior receipt reference. |
| `activate` | Approved BRR → effective; activation time is this event's `at`. |
| `reject` | Proposed ADR → rejected. |
| `deprecate` | Effective BRR/approved ADR → deprecated. |
| `retire` | Effective BRR → retired. |
| `supersede` | Explicit complete group replacement; no standalone inference. |
| `link`, `unlink` | Preserve status and frozen body while adding/removing directed related references. |

Repeated approval cannot replace a digest. An edit/reset clears the active
receipt but retains the old event. Reapproval creates a new receipt. Reset
requires a non-baseline event branch, no prior activation/completed replacement,
and no appearance of the ID in the recorded baseline commit's full history,
including earlier proposals and deleted files. A no-op cannot reset approval.
Once configuration is committed, v1 rejects a different baseline setting in
current or retained configuration (`baseline_configuration_changed`). There is
no baseline-migration command; manually retargeting the baseline cannot unfreeze
records already known to be durable.
Draft/proposed bodies remain editable without refreshing an approval digest.
Rejected/withdrawn revisions cannot reopen or silently change frozen bodies.

## Exact bytes and retained history

SHA-256 uses exact UTF-8 bytes, preserving comments, whitespace, and LF/CRLF.
BRR hashes cover the complete body after frontmatter. ADR hashes cover the
existing Markdown parser's decision body, excluding frontmatter and the final
`## Lifecycle notes` section. Whitespace before that heading remains part of
the decision body. `notes_sha256` hashes the entire excluded section (heading
included), or the empty byte string when absent.

Validation checks transition and reference consistency after every intermediate
event, including links later removed before commit; it does not compare statuses
using a shortcut from baseline to worktree. It compares terminal metadata and
active frozen revisions with current files, and validates retained record/event
snapshots in full, path-limited Git history reachable from HEAD and the resolved
baseline, including merge parents. It reads Git objects directly without
checkout, filters, hooks, fetching, or executing project code.

Current and child ledgers must retain exact parent ledger bytes as prefixes.
Current and child lifecycle notes must retain available prior notes as byte
prefixes. Missing allocated records, changed receipts, unexplained frozen-body
edits, manual statuses/links/times, and inconsistent intermediate committed
snapshots are errors. Uncommitted intermediate note text is not recoverable
from a digest; append-only text checks use retained revisions when available.
No approved-body snapshots or restoration are provided.

Divergent branch ledgers that do not preserve both parent prefixes require
explicit history/merge resolution; BADR does not combine independent ledgers.
The trusted-local model does not detect deliberate coordinated rewrites of
records, evidence, and all retained history, or authenticate human declarations.

## Complete replacement and references

A group event has a `replacement` object with nonempty, distinct
`predecessors` and `successors`, `complete: true`, and, for BRRs,
`compatibility_confirmed: true`. It affects exactly their union, all of one
record type. The declaration records human compatibility review; validation
does not judge its semantic completeness.

ADR `decide` can approve proposed successors and supersede approved/deprecated
predecessors together. BRR `activate` can activate previously approved
successors and supersede effective predecessors together. `supersede` requires
already approved ADR/effective BRR successors. All links must be reciprocal,
retain earlier replacement links, and form an acyclic graph. Splitting and
consolidation use the same explicit sets. Later successor withdrawal preserves
links and never revives a predecessor. Group changes cannot alter unrelated
links or frozen reasoning.

Ordinary related links stay directed and may form cycles; they never imply
supersession. Targets missing inside an available managed collection are
errors. References into an absent supporting collection are `unverified_reference`
(exit 3), including retained snapshots whose supporting collection was absent.
Validation does not silently import unrelated repository files.

## Coverage and diagnostics

`collection` reports structural checks; `evidence` reports local event replay,
current revision matches, and reference verification; `history` reports available
retained revision/append-only/reset checks. A populated valid collection can
now return exit 0. Missing/malformed/inconsistent evidence returns exit 2;
missing/shallow history or unverified supporting references returns exit 3;
operational I/O failure returns exit 4. Incomplete/failed prerequisite checks
cannot produce complete downstream coverage. Error precedence is unchanged.

Evidence diagnostics use `.badr/evidence.jsonl` with 1-based stored-file line
numbers for event decoding/replay failures; record diagnostics include the
record path and ID. History diagnostics identify the retained commit when
applicable. Human diagnostics use stderr; JSON diagnostics remain on stdout.
Validation never writes, changes status, refreshes hashes, or repairs evidence.

## Domain registration events

`domain_add` has `domain: {"slug": "ordering", "prefix": "ORD"}` and an empty
`changes` array: registration does not allocate a record ID. It requires explicit
confirmation, actor, rationale, actual UTC time, named current branch, and the
baseline commit resolved at operation start. It cannot carry a replacement set.
A domain slug/prefix may be registered only once in the retained event sequence.
Current and historical snapshots must retain every evidenced domain and its
glossary. Glossary content is editable; registration does not freeze its meaning.
Available registries across local refs permanently reserve prefix ownership,
including registries subsequently deleted from a branch. Registration refuses
reassignment even when the current registry no longer mentions that prefix.
The local-history guarantee cannot see unseen clones or deliberately erased refs.
