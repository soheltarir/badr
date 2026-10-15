# BADR: Rust CLI specification

Status: Draft specification; shared requirements confirmed on 2026-10-15.
Implementation status: Not started. This delivery creates only this document.

## 1. Purpose and project boundary

BADR is a standalone Rust command-line tool for managing Business Rule Records
(BRRs) and Architecture Decision Records (ADRs). It makes record creation,
approval, applicability, relationships, and integrity checks deterministic.
Developers and coding agents supply content and authorize decisions; BADR
manages the resulting files and evidence.

The project lives separately from `agent-ops-kit`. Its Rust library is internal
implementation code, without a supported public library API. Kata is design
provenance, not a runtime dependency. This specification neither changes kata
nor replaces its specification, skills, validators, or pending tickets.

V1 serves developers and agents locally on macOS and Linux. It does not include
AI interviews/generation, delivery planning, implementation-conformance analysis,
a hosted service, MCP, CI-specific integration, or a full-screen terminal UI.
Windows support, automatic adoption/relocation, legacy conversion, distributed
ID reservation, and recoverable multi-file transactions are deferred.

## 2. Authority and compatibility

Git-managed BRR/ADR files are the current-state source of truth. Configuration
and automatically generated integrity evidence are also tracked in Git. Evidence
records how BADR changed state; it is not a second editable record registry.
There is no database, background daemon, or required external service.

BADR retains kata's BRR YAML/Gherkin structure and MADR-derived ADR structure,
but the confirmed requirements intentionally change these contracts:

| Area | BADR contract |
|---|---|
| Storage | Configuration and records live under `.badr/`, not kata's documentation roots. |
| ADR approval | Persist `status: approved`; do not persist kata's `accepted` status. |
| Pre-baseline approval | Eligible branch-local approvals can explicitly reset to `proposed`. |
| Evidence | Commands maintain tracked transition evidence and body digests automatically. |
| Historical baseline | Governed writes require an explicitly configured, usable baseline branch. |
| BRR creation | Register drafts when recorded; initialization does not wait for approval. |

Exact compatibility with kata's existing validators is therefore not promised.
In particular, a kata ADR with `accepted` is not a valid BADR ADR without an
explicit future conversion path. V1 has no importer, automatic status alias,
record relocation, or legacy rewriting. Do not scan unrelated repository files
into the managed collection or treat deferred adoption as completed.

## 3. Repository layout and initialization

The fixed managed layout is:

```text
target-repository/
└── .badr/
    ├── config.yaml
    ├── evidence.jsonl
    ├── brrs/
    │   ├── registry.yaml
    │   └── domains/
    │       └── ordering/
    │           ├── glossary.md
    │           └── rules/
    │               └── BRR-ORD-001-order-cancellation.feature
    └── adrs/
        └── 0001-storage-choice.md
```

The minimum versioned configuration is:

```yaml
version: 1
baseline_branch: main
```

`baseline_branch` is an explicit choice supplied during initialization; `main`
is the recommended value, not an assumption about every repository. Arbitrary
record roots are outside v1. `init` requires a Git repository, creates only
configuration as needed, and is safe to rerun. It must not overwrite conflicting
configuration, move records, edit agent instruction files, initialize Git, or
commit. A missing/unborn baseline is reported with instructions to establish it;
initial configuration is allowed, but governed writes remain blocked.

Create record subdirectories lazily. Domain registration may initialize an empty
BRR registry and glossary; recording the first BRR registers its ID and path.
Empty collections are valid. Evidence is created with the first recorded
operation. Lock/scratch files are operational, are not durable evidence, and
must not be included in normal record commits.

## 4. Record formats

### 4.1 BRRs

A BRR is one UTF-8 `.feature` file with leading `---`-delimited YAML frontmatter.
Extract and validate frontmatter before passing only the body to an established
Gherkin parser. Require one Feature containing exactly one named Rule and at
least one named scenario or populated scenario outline belonging to that Rule.
Feature tags must agree with the BRR ID and domain.

```gherkin
---
id: BRR-ORD-001
domain: ordering
status: proposed
owners:
  - Product
  - Engineering
---
@brr:BRR-ORD-001
@domain:ordering
Feature: Order cancellation
  Rule: Customers may cancel only before fulfilment begins
    Scenario: Cancel before fulfilment
      Given a confirmed order has not entered fulfilment
      When its authorised customer requests cancellation
      Then the order becomes cancelled
```

Required metadata: stable `id`, domain slug, `status`, and nonempty descriptive
`owners`. IDs match `BRR-<PREFIX>-<number>`, with an uppercase domain prefix and
at least three digits. Filenames are `<id>-<lowercase-title>.feature`.
Optional metadata comprises `description`, `effective_from`, `effective_until`,
`related_architecture_decisions`, `supersedes`, and `superseded_by`.
Relationship values are lists of actual stable IDs; ADR IDs are quoted strings.

The registry has `version: 1`, `domains` mapping unique slugs to unique uppercase
prefixes, and `rules` mapping IDs to paths relative to `.badr/brrs/`. Empty
mappings are valid during initialization. Each registered domain has a glossary;
each registered rule belongs beneath its domain's `rules/` directory. Every
managed feature file must be registered. Prefixes are never reassigned.

BRRs express observable product behavior, policies, invariants, events, time,
and exceptions. They do not prescribe frameworks, storage, protocols, vendors,
or deployment topology. Syntax checks do not prove that content is a sound
business rule or that its scenarios are sufficient.

### 4.2 ADRs

An ADR is one UTF-8 Markdown file directly under `.badr/adrs/`, named
`NNNN-lowercase-title.md`. IDs are quoted numeric strings padded to at least
four digits, match filenames, and share one repository-wide numeric sequence.
Different zero padding does not create distinct numeric identities. There is
no separately maintained ADR registry/index.

```yaml
---
id: "0001"
status: proposed
date: "2026-10-15"
decision-makers:
  - Engineering
related_business_rules:
  - BRR-ORD-001
---
```

Required frontmatter: `id`, `status`, and quoted `date` in `YYYY-MM-DD` form.
The date records the latest governed update, not an invented historical
approval date. Optional fields are `decision-makers`, `supersedes`,
`superseded_by`, `related_adrs`, and `related_business_rules`; relationship
fields are lists, empty when omitted. Descriptive owners/decision-makers do
not establish authorization.

Bundle a BADR-owned adaptation of the MADR 4.0.0 template, retaining provenance
and the CC0-1.0 declaration from the reference adaptation. Required reasoning
sections are Context and Problem Statement, Decision Drivers, Considered
Options, Decision Outcome with rationale, and Consequences. Detailed option
analysis and Confirmation remain optional. Proposed reasoning may contain
explicit unresolved items; approval requires the caller to confirm review
and resolution of applicable gaps. Headings alone do not prove adequacy.

The decision body excludes frontmatter and an optional final
`## Lifecycle notes` section. That section contains dated, append-only lifecycle notes;
it cannot conceal reasoning changes or be moved to exempt decision text.
Headings inside code fences do not create template or lifecycle sections.

## 5. Lifecycles, authorization, and applicability

### 5.1 Common approval command

`badr decide <id>` is the explicit approval operation for both record types.
It requires `proposed`, validates the applicable content/relationships, obtains
an explicit caller confirmation plus actor and rationale, records the actual
UTC time and resolved baseline commit, captures a body digest, and stores
`status: approved`. It never infers approval from descriptive metadata or from
successful validation. Repeating it on an already approved record is not a new
approval or permission to replace the digest.

Humans review the rule/decision before authorization. An agent must obtain
human authorization outside BADR. BADR records that declaration rather than
authenticating an approver or proving an interview happened. There is no
separate manual ledger-management or mandatory ledger-review workflow.

### 5.2 BRR lifecycle

```text
draft -> proposed -> approved -> effective
                                  |-> deprecated
                                  |-> superseded
                                  `-> retired
```

`decide` approves but does not activate. `activate` is a separate explicit
human choice and sets `effective_from` to the actual immediate activation time.
Only effective BRRs are currently applicable. Deprecation, supersession, and
retirement immediately end applicability and retain the activation time while
setting `effective_until`. Times are quoted ISO 8601 with seconds and an
explicit timezone, normalized to UTC for new events. Future scheduling and
backdated new activations/withdrawals are outside v1.

Draft/proposed/approved records have no applicability times. Withdrawal does
not revive another record or imply a replacement; surface policy gaps for
explicit human resolution. Withdrawn bodies remain frozen and records are not
reopened. A changed durable rule requires a successor, except for the narrow
pre-baseline edit/reset in section 6.

### 5.3 ADR lifecycle

```text
proposed -> approved -> deprecated -> superseded
    |            `-----------------> superseded
    `-> rejected
```

There is no ADR draft, `accepted`, or separate activation state. An approved
ADR immediately directs new work in the checked-out repository context.
Approval does not assert that existing code conforms. Rejection is an explicit
human choice and remains non-authoritative. Deprecation immediately withdraws
direction for new work and records rationale, any decision gap, and treatment
of existing code without changing that code. Rejected/withdrawn records are
not reopened; a deprecated ADR may later be superseded.

Governed lifecycle/relationship changes require explicit confirmation, actor,
and rationale. ADR changes update `date` and append lifecycle notes as relevant.
Metadata-only updates must preserve frozen meaning; changing reasoning needs
a successor unless section 6 permits an explicit reset. Applicability follows
the checked-out branch; approval on a feature branch does not rewrite baseline
files or claim production implementation conformance.

### 5.4 Supersession and relationships

Completed replacement supports one successor replacing several predecessors
and several successors jointly replacing one predecessor. Require explicit
human declaration of complete replacement, reciprocal `supersedes` /
`superseded_by`, valid targets, legal statuses, and an acyclic replacement graph.
Intended or partial replacement stays in reasoning and ordinary related links.
Ordinary related links are directed references, not implicit supersession.

All ADR successors must be approved when replacement is authorized. BRR
successors require prior approval and explicit activation to supply replacement
policy; approval alone cannot withdraw predecessors. Successor BRRs must
document compatibility treatment for existing data and in-flight behavior
(grandfather, migrate, re-evaluate, or reject), with observable scenarios,
before approval. BADR checks required structure and the review declaration;
humans judge semantic completeness.

Support an explicitly authorized group change for ADR approval plus replacement,
or BRR activation plus replacement after prior approval. Never infer a
replacement set from approval or a related link. Validate the intended whole
group before writing. Later withdrawal of successors preserves historical
links and does not revive predecessors. Group changes have the partial-write
limitation in section 9; they are not crash-atomic transactions.

## 6. Branch-local editing after approval

Approval freezes the body by default. `badr edit <id>` may reset an approved
record to `proposed` only when all of these hold:

1. The current checkout is a named branch other than the configured baseline.
2. The ID has never appeared in the baseline branch's history, including an
   earlier draft/proposal. Looking only at the current baseline tree is insufficient.
3. The record has never been activated and does not participate in completed
   supersession, either as predecessor or successor.
4. The baseline/history and existing evidence are available and valid.

If an eligible body changes through explicit `edit`, record the reset and prior
approval reference, set `proposed`, update ADR date as applicable, and retain
all earlier evidence. Require another `decide` to approve the new revision.
An editor no-op does not reset approval. The same eligibility gates apply when
explicitly registering a body change made in an external editor. That operation
may resolve the target's digest mismatch, but must not bypass unrelated errors,
silently approve content, or discard previous evidence.

Validation is read-only: it never changes status merely because it detects an
edit. Unauthorized direct edits remain errors until explicitly registered when
eligible or restored when ineligible. Draft/proposed content is normally
editable; lifecycle changes still use BADR commands. Allowed metadata edits
must satisfy identity, evidence, relationship, and frozen-meaning constraints.

Once an ID has appeared in baseline history, approval cannot be reset even
on later feature branches. Branch renaming, subsequent deletion, successor
withdrawal, or baseline reconfiguration must not unfreeze an ID already known
to be durable. Require a successor for durable reasoning/policy changes.
Detached checkouts and uncertain/incomplete history do not qualify for reset.

## 7. Baseline, allocation, and automatic evidence

### 7.1 Explicit baseline

The configured branch supplies the explicit baseline; users need not repeat a
commit argument on each command. Resolve its locally available tip once at
operation start and record the immutable commit ID. Refuse governed writes
when it cannot be resolved, required history is incomplete, or the reference
changes before the write can safely proceed. Do not fetch automatically.

History is required for approval/reset eligibility, lifecycle/freeze checks,
and ID allocation. Read snapshots without checking them out or executing
anything from them. Read-only validation may still report structural/local
evidence results when history is unavailable, but historical coverage is
incomplete and the overall result must not claim complete success. Initial
configuration is not an approval and may precede establishing the baseline.

### 7.2 IDs

Allocate above the highest reserved number for each BRR domain prefix or the
ADR sequence, considering local records, retained allocation evidence, and
locally available Git history. Record allocation under the write lock and use
exclusive creation. Never reuse allocated, rejected, withdrawn, or deleted IDs,
and never silently renumber frozen records. Deleting an uncommitted proposal
does not release its recorded ID. Removing a required record is an integrity
error, not a supported lifecycle command.

Guarantees cover the local repository and available evidence, not unseen
clones. Separate clones may allocate colliding IDs; validation after merging
must reject collisions and require explicit human resolution. Distributed
reservation and automatic conflict resolution are deferred.

### 7.3 Evidence

`.badr/evidence.jsonl` is a versioned, append-only sequence of operation events.
Events capture operation identity/type, affected record IDs, prior/resulting
states or allocation, actor/rationale and confirmation declaration for governed
changes, actual UTC time, current branch, resolved baseline commit, and body
digest/revision references when approval/reset requires them. Group events
identify the complete intended replacement set. Reapproval creates new
evidence; it never rewrites the original approval receipt.

Use SHA-256 over exact UTF-8 body bytes, preserving comments, whitespace, and
line endings. Hash the full BRR body; hash the ADR decision body as defined in
section 4.2. Validate append-only lifecycle notes separately. Do not put
evidence-only fields into compatible record frontmatter. Full approved-body
snapshot storage is outside v1: a digest detects a change but cannot restore text.

Commands automatically validate evidence before mutation. History checks replay
permitted evidenced transitions, rather than rejecting valid intermediate
steps merely because a baseline-to-working-tree comparison skips them. Match
the current approved revision to its active approval receipt. Missing evidence,
manual status changes, or unexplained body changes cannot be silently repaired
by blessing current content.

Git preserves the durable audit trail after the user commits records and
evidence. Until then, report pending commit durability. Prepare an accurate
audit commit message; never commit automatically. No independent ledger
reviewer is required. Deliberate coordinated rewriting of records, evidence,
configuration, and history is outside this trusted-local integrity model;
authenticated approvals and tamper-proof audit are not claimed.

## 8. CLI and agent interface

The following command families are required; detailed flag spelling belongs
to implementation, while the described operations are the v1 contract:

| Command family | Required behavior |
|---|---|
| `init` | Configure fixed layout and explicit baseline safely. |
| `domain add` | Register a unique BRR domain/prefix and glossary scaffold. |
| `new brr`, `new adr` | Allocate/register identity; create draft BRR or proposed ADR from bundled template or supplied content. |
| `propose` | Move a structurally valid BRR draft to proposed. |
| `edit` | Accept supplied/editor content; perform eligible explicit approval reset when needed. |
| `decide` | Approve either type; support explicit group approval/replacement as applicable. |
| `activate` | Activate approved BRRs separately; optionally execute an explicitly declared complete replacement set. |
| `reject` | Reject proposed ADRs. |
| `deprecate`, `retire` | Withdraw authority/applicability; retirement applies to effective BRRs. |
| `supersede` | Explicitly replace eligible predecessors with qualified successors. |
| `link`, `unlink` | Manage permitted related links; never bypass replacement governance. |
| `list`, `show`, `related` | Inspect records, applicability, and explicit relationship traversal. |
| `validate` | Read-only collection, evidence, and historical checks with coverage reporting. |

List/filter by type, domain where applicable, and status. Provide current
effective BRRs and approved ADRs, and explicitly labeled pending-rule previews.
This selects records; it does not assess implementation conformance. Semantic
search, embeddings, and a general query language are outside v1.

Every core operation works with explicit arguments/content inputs, without
required prompts or editor launches. Optional editor-assisted authoring is
explicitly requested. Never execute project-supplied scripts or tool hooks to
read/validate records. Human-readable output is default; JSON mode must expose
a documented schema version, success/failure, operation/result, affected IDs,
structured diagnostics, baseline commit, validation coverage, pending audit
durability, and partial-write details when relevant.

JSON output must have deterministic ordering and predictable documented exit
codes distinguishing success, invalid input/integrity failures, incomplete
required coverage, and operational failures. No terminal prose may corrupt
JSON stdout; human diagnostics belong on stderr. Exact enum/exit-code values
must be documented before the first release and tested as an agent interface.
Do not describe an incomplete historical check as passed or use a zero success
result for an operation whose required checks could not be completed.

## 9. Validation and write safety

Use established YAML and Gherkin parsers; do not claim full syntax support from
a handwritten subset. Reject duplicate/unknown metadata keys, unsafe YAML
tags, invalid types/times, duplicate IDs/paths/prefixes, filename mismatches,
unregistered/missing records, invalid references, graph cycles, illegal
transitions, unauthorized body edits, and inconsistent evidence. Diagnostics
identify the file/record and distinguish body from stored-file line numbers.

An absent record collection is valid, not a reason to block unrelated product
development. References to absent supporting collections are reported as
unverified; missing targets inside an available managed collection are errors.
Validate the entire managed collection before and after mutations. Explicit
eligible edit/reset may resolve its target's known body mismatch; it is not a
general validation bypass. Existing invalidity blocks other managed mutations.

Resolve paths within the target Git repository and fixed `.badr/` roots. Refuse
symlink traversal, path escapes, unexpected file replacement, and overwrite of
unrelated data. Use a repository-local single-writer lock, exclusive creation,
and atomic replacement of each individual file. Check expected file contents
before replacement to detect external edits. A stale lock requires a documented,
explicit safe cleanup procedure; do not blindly remove another writer's lock.
The lock coordinates BADR processes; arbitrary editors do not honor it.

Prevalidate the intended complete change, including registry, relationships,
and evidence. Then write and revalidate. This is not atomic across files.
On failure, report exactly which writes completed and which did not, stop,
and block further mutations while the collection is inconsistent. Manual
repair restores consistency from known evidence/Git/worktree information;
BADR must not invent authorization, refresh a digest to hide damage, or discard
unrelated uncommitted changes. Known partial failure must remain distinguishable
from success even if the remaining files happen to parse.

**Flagged v1 risk — deferred recoverable transactions:** power loss or another
mid-operation failure can leave partial multi-file changes, including incomplete
supersession or mismatched record/evidence files. There is no transaction
journal, automatic rollback, or guaranteed crash recovery. Preserve this as
explicit technical debt and revisit before claiming crash-atomic group changes.

## 10. Distribution and runtime

Ship versioned downloadable archives and source-build instructions for:

| Operating system | Binary architectures |
|---|---|
| macOS | Apple Silicon / ARM64; Intel / x86_64 |
| Linux | ARM64; x86_64 |

Publish checksums and maintain BADR's own Homebrew tap selecting the appropriate
release archive. Homebrew/core acceptance and a separate bottle-build pipeline
are not release prerequisites. Homebrew installation is subject to upstream
host support; it does not establish BADR's own minimum macOS/Linux runtime
baseline or promise continued Intel macOS Homebrew availability.

BADR's runtime is the installed binary plus trusted Git for managed operations;
an editor is optional. Python, Go, kata skills, credentials, and network access
are not required for ordinary local management. Source builds require a
documented Rust toolchain. Pin/document actual parser dependencies, minimum
Rust/OS/Linux runtime requirements, archive layout, and installation commands
during implementation. Verify these on release targets before claiming support.
Homebrew repository ownership, signing credentials, and publication are release
operations, not authorized or performed by this specification delivery.

## 11. Acceptance and implementation verification

V1 is complete only after meaningful checks demonstrate:

1. Safe repeated initialization, configured non-`main` baseline, valid empty
   collections, lazy creation, domain registration, and no unrelated writes.
2. BRR frontmatter extraction/grammar/cardinality and ADR section/type checks,
   including duplicate keys, unsafe tags/paths, malformed links, and ID collisions.
3. Both `decide` operations store `approved`; BRR approval remains inactive,
   ADR approval directs new work, and declarations/evidence are required.
4. Edit/reset works only for eligible pre-baseline approved records, retains
   previous evidence, and requires reapproval. Test direct edits, editor no-op,
   baseline-history inclusion of an earlier proposal, detached checkout,
   deletion, and attempted reset after activation/supersession.
5. Exact body-byte changes are detected, ADR lifecycle notes remain append-only,
   and authorized metadata edits preserve approved reasoning/behavior.
6. Immediate activation/withdrawal, compatibility scenarios, consolidation,
   splitting, coordinated replacement, reciprocal links, cycle rejection,
   partial replacement refusal, and non-revival after successor withdrawal.
7. Baselines resolve to recorded commits; missing/shallow/unusable history
   blocks governed writes. Read-only reports separate incomplete coverage.
   Multiple valid precommit transitions are checked using retained evidence.
8. Local concurrent allocation, never-reused/deleted IDs, retained allocation
   events, and merged-clone collisions behave as documented.
9. Failure injection between individual writes reports partial outcomes and
   blocks further mutation until manual repair; no automatic recovery claim.
10. JSON/exit contracts, caller-supplied content, explicit optional editor use,
    and execution without prompts work for agents.
11. Native build/smoke verification on every claimed binary target, source-build
    instructions, archive/checksum verification, and own-tap installation work.

Use the same internal Rust logic for CLI operations and validation. Do not
create a public library compatibility layer, daemon, plugin system, or custom
parser framework. A practical delivery order is layout/parsing/querying,
evidence/baseline/allocation, authoring/decide/edit, remaining lifecycle and
replacement operations, then release packaging. Partial milestones must not
be described as the complete v1 lifecycle.

No implementation, runtime, packaging, or cross-platform checks have been
performed for BADR by drafting this document. Shared requirements confirmation
authorizes this specification, not an implementation or release.

## 12. Reference provenance

Local references below point to the sibling `agent-ops-kit` checkout, inspected
at commit `faaae5d97386941810bb1a7ac319aafd753c95fd`. They explain origins; BADR
must not load them at runtime or silently inherit later edits. Related local
tickets were untracked planning artifacts and include planned behavior, not
proof of implemented kata capabilities.
Where they conflict, this confirmed BADR specification controls BADR.

- [Domain-driven SDLC specification](../../../agent-ops-kit/docs/specs/domain-driven-sdlc-spec.md).
- [Kata pack status](../../../agent-ops-kit/packs/kata/README.md).
- [BRR format/lifecycle reference](../../../agent-ops-kit/packs/kata/skills/kata-brr/references/authoring.md),
  [metadata](../../../agent-ops-kit/packs/kata/skills/kata-brr/assets/metadata.yaml),
  and [registry example](../../../agent-ops-kit/packs/kata/skills/kata-brr/assets/registry.yaml).
- [ADR authoring reference](../../../agent-ops-kit/packs/kata/skills/kata-adr/references/authoring.md),
  [template](../../../agent-ops-kit/packs/kata/skills/kata-adr/assets/adr.md),
  and [template provenance](../../../agent-ops-kit/packs/kata/skills/kata-adr/references/template-provenance.md).
- [BRR successor requirements](../../../agent-ops-kit/.scratch/kata/issues/05-successor-brrs.md)
  and [ADR replacement requirements](../../../agent-ops-kit/.scratch/kata/issues/08-supersede-adrs.md)
  are local planning references, not required BADR project files.
- [Kata ADR research](../../../agent-ops-kit/docs/specs/kata-adr-management-research.md)
  and [reference research](../../../agent-ops-kit/docs/specs/kata-reference-research.md).
- ADG's [decide implementation](https://github.com/adr/ad-guidance-tool/blob/3f35fd65a11c888fdc4da76a8de509b8be3373c9/internal/application/interactor/decision/decide.go)
  and [edit implementation](https://github.com/adr/ad-guidance-tool/blob/3f35fd65a11c888fdc4da76a8de509b8be3373c9/internal/application/interactor/decision/edit.go)
  informed the command interface. BADR's digest enforcement and branch reset
  rules are its own contract, not a claim about ADG immutability.
- Official Homebrew [tap guide](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap),
  [formula cookbook](https://docs.brew.sh/Formula-Cookbook), and
  [support tiers](https://docs.brew.sh/Support-Tiers) inform distribution.
  Recheck platform policy when implementing releases.
