# BADR CLI contract, version 1

BADR implements initialization and read-only BRR/ADR inspection and structural
validation, operation-evidence replay, exact frozen-body checks, and retained
Git revision checks, governed domain registration, record creation, BRR proposal,
ordinary record editing, explicit approval, eligible branch-local approval reset,
ADR rejection/deprecation, immediate BRR activation/withdrawal, explicit
complete BRR replacement, and coordinated ADR replacement.
See the [versioned evidence contract](evidence.md) for the event schema and
coverage limits.

```sh
badr --repo /path/to/existing/repository init --baseline trunk
badr --repo /path/to/existing/repository list
badr --repo /path/to/existing/repository validate --json
```

`--repo` defaults to the current directory. A directory inside a working tree
selects that working tree's root. `--json` and `--repo` may precede or follow
the command. No operation prompts or fetches. Only explicit `edit --editor`
launches an editor; no operation runs project scripts, initializes Git, stages
files, or commits.

`init` requires `--baseline <local-branch>`; `main` is a recommendation only.
It writes only `.badr/config.yaml`, creating `.badr/` if necessary. Matching
configuration is preserved byte for byte. Different, malformed, or unsupported
configuration is refused. Unknown/duplicate keys, invalid field types, and
custom YAML tags are rejected. Nonempty unconfigured `.badr/` storage is
refused. Managed symlinks and special files are refused.

A missing/unborn baseline still permits configuration creation, but returns
incomplete coverage (exit 3). Create the chosen local branch and its initial
commit, then rerun validation. BADR does not assume another branch or fetch.
This exception enables no governed record writes.

`list`, `show <id>`, and `validate` are read-only and share collection
inspection logic. All operations inspect the whole managed collection before
selection, so a filter cannot hide invalid records. Absent record directories
and evidence, empty record directories, empty registry mappings, and an empty
evidence file are valid. Populated collections require consistent operation
evidence, including allocation and active approval receipts.
Unrelated files outside `.badr/` are not collected. Baselines resolve once to
an immutable local commit ID; shallow history, unavailable Git objects, or
missing baselines prevent complete historical coverage. No snapshot is checked out and no history content is executed.

BRR frontmatter and registry YAML reject duplicate/unknown keys, unsafe tags,
malformed types, and invalid UTF-8. Only the exact remaining body is passed to
`gherkin` 0.14.0. Checks cover Feature/Rule/scenario cardinality and names,
populated outlines, matching Feature tags and metadata, owners, relationship
list types and ID formats, filenames, registry identities/paths/prefixes,
domain-local placement, glossaries, and registration of managed features.
Managed symlinks, special files, path escapes, and unexpected storage layouts
are refused. Validation never edits records, configuration, or evidence.

Draft/proposed/approved BRRs have no applicability times; effective BRRs require
`effective_from` and no `effective_until`; deprecated/superseded/retired BRRs
retain activation and require an end time no earlier than activation. Timestamp
strings require quoted YAML scalars, ISO 8601 seconds, and an explicit timezone.
`saphyr-parser` events check scalar quoting, which Serde values do not retain. Future events are
invalid. These checks establish structural lifecycle consistency, not an
authorized historical transition. Owners are descriptive, never authorization.

`list` supports `--type brr|adr`, `--domain <slug>`, `--status <status>`,
`--effective`, and `--pending-preview`. The last two flags are mutually
exclusive; other filters intersect. All BRR statuses are accepted as status
filters. Without flags, list returns all locally parsed records. Each record is
explicitly labeled `effective`, `pending_preview` (draft/proposed/approved), or
`withdrawn`. Only effective records are currently applicable. Human `show`
prints the selection and exact body; JSON records include all validated
metadata and the exact body. An absent or invalid ID produces `record_not_found`. Invalid records may be omitted;
read the coverage and diagnostics before using a partial inspection result.

Populated collections can pass evidence/history validation when their retained
events, frozen revisions, relationships, and available historical snapshots are
consistent. Missing evidence returns exit 2. Absent supporting collections leave
references explicitly unverified (exit 3); missing targets in available
collections are errors. Passing checks never proves semantic adequacy,
authenticated human approval, or implementation conformance.

## ADR inspection


```sh
badr list --type adr --status proposed
badr list --directing --json
badr show 0001 --json
```

`--type` accepts `adr` or `brr`. `--status`
accepts `proposed`, `approved`, `rejected`, `deprecated`, and `superseded`.
`--directing` selects approved ADRs as directing new work in this checkout;
it does not assert that existing code conforms or that approval evidence was
verified. Filters combine, and malformed/unselected records still affect the
result. `show` takes an exact quoted metadata ID (including padding), never a
path; absent/invalid records return exit 2.

Each record contains `type`, `id`, `status`, `date`, `path`, `decision-makers`,
relationship arrays, `directs_new_work`, and `body_start_line`. Only `show`
also includes `decision_body`, the exact UTF-8 decision bytes decoded as a JSON
string (including whitespace, comments, and original LF/CRLF line endings).
Human `show` prints that body after the record summary. Frontmatter and the
optional final lifecycle-notes section are excluded; whitespace before the
lifecycle heading remains part of the decision body. No ADR index is created.

ADR files must be directly under `.badr/adrs/`, with numeric ID and lowercase
hyphen-separated title filenames. IDs require at least four digits and exact
filename/frontmatter agreement; alternate padding still collides numerically.
YAML uses the established `yaml_serde` parser; `yaml-rust2` scans quote styles
and explicit tags. Required `id` and calendar `date` must be quoted; ADR
relationship IDs must also be quoted. Duplicate/unknown keys, all explicit
tags, invalid UTF-8, invalid types/statuses/dates, and unsafe paths are errors.
Omitted relationship fields are empty lists; explicit null is invalid.
Decision-makers, when present, are a list of nonempty descriptions.

Required sections are level-two Context and Problem Statement, Decision
Drivers, Considered Options, and Decision Outcome, plus level-two Consequences
or level-three Consequences under the outcome. Sections need content, with
rationale text under Decision Outcome (a Rationale subsection is allowed).
Explicit unresolved items are allowed; wording and adequacy require human review. Detailed option analysis
and Confirmation are optional. CommonMark parsing ignores fake headings in
fences, indented code, and nested blockquotes/lists.

An optional `## Lifecycle notes` must be the final section, containing one or
more `### YYYY-MM-DD` headings (optionally followed by a title) with nonempty
note text. It cannot precede later decision sections. These are structural
checks. Evidence validation additionally checks retained digests and append-only
notes against available revisions. Headings alone cannot prove human authorization
or semantic adequacy.

## Output and exits

Human summaries use stdout and human diagnostics use stderr. `--json` writes
one JSON object and a newline to stdout; diagnostics are inside that object,
with no terminal prose on stderr. Usage errors also follow this contract,
with operation `arguments`. `--help` and `--version` are text informational
options, rather than JSON operations.

| Exit | Meaning |
| --- | --- |
| 0 | Operation succeeded with its required checks complete |
| 2 | Invalid arguments/configuration, unsafe managed paths, or integrity failure |
| 3 | Required validation coverage is incomplete; never complete success |
| 4 | Operational failure (I/O, Git execution, or writer lock) |

Invalidity takes precedence over incomplete coverage; operational failure takes
precedence over both. Initialization may return exit 3 after writing its
configuration. Read `partial_write.completed` to see completed writes even
when `success` is false.

The [JSON Schema](json-schema-v1.json) specifies every version 1 field. Object
field order is fixed, diagnostics follow deterministic path/check order, and
there are no timestamps or random operation identifiers in these read reports.
The same repository state produces the same JSON bytes.

- `schema_version`: integer `1`; breaking contract changes require a new version.
- `operation`: `init`, `domain_add`, `new_brr`, `new_adr`, `propose`, `edit`, `decide`,
  `activate`, `supersede`, `reject`, `deprecate`, `retire`, `link`, `unlink`, `related`, `list`,
  `show`, `validate`, or `arguments`.
- `success`: true only for exit 0.
- `result`: `created`/`unchanged` for successful init, `created` for registration/creation, `proposed` for BRR proposal, `edited`/`unchanged` for editing, `approved` for approval, `effective`/`deprecated`/`retired`/`superseded` for BRR lifecycle changes, `rejected`/`deprecated` for ADR withdrawal, `ok` for successful
  inspection, otherwise `invalid`, `incomplete`, or `operational_failure`.
- `exit_code`: the process exit code.
- `affected_ids`: the created/proposed/edited/approved/activated/withdrawn/replaced record IDs after complete-change planning;
  empty for inspection, initialization, domain registration, and edit no-ops. Partial failures
  retain the intended IDs; inspect `partial_write` to establish what was written.
- `records`: BRR objects ordered by ID then path. Each has `type: "brr"`,
  `id`, `domain`, `status`, `owners`, relationship lists, optional description
  and applicability times, managed-relative `path`, Feature `title`,
  `selection`, `currently_effective`, and exact UTF-8 `body` (JSON-escaped).
  ADR objects use the fields described above. Mixed results are ordered by ID
  then path. An empty array with incomplete coverage does **not** assert an empty collection.
- `diagnostics`: ordered objects with `code`, `message`, and managed-relative
  `path` when applicable (otherwise null), `record_id`, `stored_file_line`, and
  `body_line` (null when not available). Structured ADR line numbers are 1-based,
  with stored-file lines including frontmatter and body lines excluding it.
  Messages are human guidance;
  consumers should branch on exit/coverage and diagnostic codes. Gherkin
  messages label body-relative and stored-file line numbers separately.
  Frontmatter YAML locations start at input line 1 / stored-file line 2;
  registry YAML locations are file-relative. Human diagnostics include paths.
- `baseline_commit`: resolved immutable commit string, or null if unavailable.
- `validation_coverage`: `configuration`, `collection`, `evidence`, `history`,
  each `passed`, `incomplete`, or `not_checked`. Collection `passed` means
  structural checks passed. Evidence `passed` means local replay, current
  revisions, and references were checked; history `passed` means retained
  snapshots/append-only/reset checks completed against available trusted local
  history. Neither status authenticates an actor or judges semantic adequacy.
- `pending_commit_durability`: true when `.badr/` has staged, unstaged,
  untracked, or ignored changes; false when clean; null when undetermined.
  A true result requires the caller to review and commit configuration;
  unrelated working-tree changes do not affect this flag.
- `partial_write`: `completed` and `not_completed` arrays of managed file
  paths. These record writes performed or attempted by this invocation;
  both arrays are empty for read-only operations and unchanged init.
  For domain registration, these arrays follow publication order (glossary,
  registry, evidence). `not_completed` includes intended paths left unwritten
  when an operation stops; it is populated after successful prevalidation.
  Directory scaffolding may remain after a failed publication.
  No transaction journal or automatic rollback is provided.
- `proposed_audit_commit_message`: null for reads, init, and failed writes;
  a proposed message describing the operation, identity/domain, baseline, event ID,
  and actor/rationale for governed operations after success. Review and commit manually.

## Writer lock and cleanup

Initialization exclusively creates repository-root `.badr.lock` and removes
it on normal completion, including ordinary errors. An existing lock prevents
writes (exit 4); reads remain available. The lock is operational scratch,
not audit evidence: do not stage it.

After a crash, first establish that no BADR process is still writing to this
working tree. Inspect `.badr/` and `git status`, preserving all unrelated changes.
Only after confirming that no active writer owns the lock, explicitly remove
`.badr.lock` and rerun validation before another initialization. Never remove
a lock to bypass an active writer. A failure before configuration publication
can leave an empty `.badr/` directory; rerunning init can reuse it. Individual
configuration publication is exclusive; multi-file crash recovery is deferred.

## Governed domain registration

```sh
badr domain add ordering --prefix ORD --confirm --actor "Human" \
  --rationale "Establish ordering vocabulary" --json
```

All arguments are explicit; no prompt or editor is required. `--confirm` records
the caller's declaration of human authorization, without authenticating it.
A slug consists of lowercase ASCII letters/digits separated by single hyphens;
a prefix contains uppercase ASCII letters. Both must be unique and permanently
retain their ownership. Repeating registration is refused, preserving files.
The command requires a named checkout and a usable baseline with complete
required history. Missing/unborn/shallow history blocks writes even when init
was allowed. The baseline is resolved once for validation/evidence and its ref
is compared with that immutable commit before each publication and at completion.

Registration validates the entire collection and evidence/history under the
repository-local writer lock. Existing unrelated invalidity blocks the command.
It prevalidates a complete temporary intended state using the same validators,
then publishes the glossary, registry, and appended evidence in that order.
The registry is created only when absent; existing rules and domains survive.
Only needed directories are created; no empty `rules/` or ADR directory is added.
Expected content and file identities detect external replacements/edits; the
complete managed snapshot is compared at every write boundary. Directory-relative
opens refuse symlink traversal. New files use exclusive publication; existing
files use atomic per-file replacement. Postwrite validation checks the entire
collection again. Unrelated uncommitted files are preserved.

Human and JSON output report completed/pending paths, pending Git durability,
and the proposed audit commit message. Neither registration nor any other BADR
command stages or commits. Review the three reported files and configuration,
then stage only `.badr/` (or explicit managed file paths). Repository-root
`.badr.lock` and `.badr-write-<pid>` are operational files, outside `.badr/`;
never stage them or use an indiscriminate `git add .` for an audit commit.
Intended-state scratch lives in the system temporary directory. If `.badr/`
lives on a different filesystem, publication fails safely rather than falling
back to a non-atomic copy.

After any failure once publication has begun, exit 4 and a retained lock mark
manual repair as required. This includes a detected postwrite problem and an
operation that wrote all files but failed its final checks. A later read-only
validation may pass; it does not erase the known failure or clear the lock.
Before-write failures publish nothing and release their own lock. The command
never removes an existing lock or a lock that replaced its own inode.

For explicit stale-lock cleanup:

1. Inspect `.badr.lock` (writers record PID, UTC start time, and operation).
   On the machine owning this working tree, inspect `ps -p <pid> -o pid,lstart,command`
   and `lsof <repo>/.badr.lock`; account for PID reuse and shared working trees.
   A missing PID alone is insufficient on a shared filesystem. Establish that
   no BADR writer is active, coordinating with other users if necessary.
2. Preserve `git status`, the failure report, lock contents, retained evidence,
   and current managed files. Inspect staged and unstaged diffs. If an operation
   was interrupted, compare every reported completed/pending path. An interrupted
   process may not have returned a report; use Git, evidence, worktree contents,
   and any root `.badr-write-<pid>` scratch to establish what happened.
3. Manually restore a consistent, known state or finish only writes for which
   the original authorization/evidence is known. Do not invent events, refresh
   digests to bless damage, or discard unrelated changes. Run `badr validate`;
   resolve all invalidity/incomplete history before proceeding.
4. Only after no writer is active and repair is reviewed, explicitly remove
   that stale `.badr.lock` and any identified stale `.badr-write-<pid>` scratch.
   Validate again before the next managed mutation. BADR never auto-cleans
   another process's lock or performs automatic rollback.

**V1 debt:** writes are not atomic across files. There is no transaction journal,
automatic rollback, or guaranteed crash recovery. A crash can leave a subset
of the intended files and directory scaffolding. Arbitrary editors do not honor
BADR's lock; expected-content checks detect observed edits but cannot provide
an atomic compare-and-replace against an editor racing the final rename.
Do not edit managed storage during a governed write. Symlink traversal is
refused by descriptor-relative directory opens even during replacement races.

Debug builds expose `BADR_TEST_FAIL_AT`, `BADR_TEST_PAUSE_AT`, and
`BADR_TEST_CONTROL` solely for deterministic executable integration checks.
Release builds ignore them. Boundary names are `before_apply`,
`before_write_0..2`, `after_write_0..2`, `before_postvalidate`, and
`validate_written`, and `after_postvalidate`; pause writes a `ready` marker in the control directory
and waits at most 15 seconds for `resume`. These controls cannot authorize
or bypass validation.


## Record creation and BRR proposal

```sh
badr new brr order-cancellation --domain ordering --owner Product --owner Engineering
badr new adr storage-choice
badr new brr supplied-rule --domain ordering --content /path/to/rule.feature --json
badr new adr supplied-decision --content /path/to/decision.md --json
badr propose BRR-ORD-001 --confirm --actor "Human" --rationale "Ready for review"
```

The title argument is a lowercase filename slug. BRR creation requires an existing
registered domain. Template BRRs require one or more `--owner` values; supplied
BRRs take owners from their content. BRRs start as `draft` and are registered
immediately; ADRs start as `proposed`, without an ADR index. Directories are
created lazily. Bundled TODO scaffolds satisfy structural validation; they do
not establish adequate business rules or completed decision reasoning.
The ADR template is BADR's adaptation of MADR 4.0.0 under CC0-1.0; see
[third-party notices](../THIRD_PARTY_NOTICES.md).

`--content` reads a complete UTF-8 file, including frontmatter, relative to the
caller's current directory (absolute paths are also accepted). Valid supplied
bytes are preserved exactly. Content must use the next allocated ID, the selected
BRR domain, the legal initial status, and valid metadata/body structure. ADR
`date` must equal today's UTC operation date. Allocation cannot establish related
or replacement links; leave these lists empty. Invalid content is refused before
publication, without reserving an ID. Creation never prompts or launches an editor.

Allocation considers current records, retained evidence, and records/evidence
through all locally referenced Git history, including other branches and earlier
deleted files. BRR numbers advance per registered prefix; ADR numbers share one
sequence regardless of padding. New numbers use at least three BRR/four ADR
digits. Arbitrarily large decimal identities remain reserved. Unseen clones
cannot coordinate allocations; merged numeric collisions are errors requiring
explicit human resolution. BADR never silently renumbers a record.

Creation uses the shared writer lock and full pre/postvalidation. The allocation
receipt is published first, then the exclusively created record, then the BRR
registry when applicable. Partial reports list this exact order. An interrupted
creation retains its receipt/reservation and lock even if the record was never
published; repair must preserve the reservation. Deleting an uncommitted proposal
is an integrity error, not a way to release its ID. Restore the known record;
BADR does not provide a delete command or bypass unrelated invalidity to allocate.

`propose` accepts only a structurally valid draft BRR. It requires explicit
`--confirm`, nonempty `--actor` and `--rationale`, records a governed event, and
moves only to `proposed`; it grants no approval or applicability. It serializes
frontmatter while preserving the exact Gherkin body, including line endings.
Publication order is record then evidence. Creation and proposal report affected
IDs, pending audit durability, and a proposed commit message; BADR never stages
or commits. All shared baseline, path, lock, expected-content, and manual-repair
rules above apply.

## Ordinary content editing

```sh
badr edit BRR-ORD-001 --content /path/to/revised-rule.feature --json
badr edit 0001 --content /path/to/revised-decision.md --json
badr edit 0001 --editor /usr/bin/vi
```

Choose exactly one of `--content` and `--editor`. Supplied content is a complete
UTF-8 record, including frontmatter; paths are relative to the caller. BADR
preserves accepted bytes exactly. Draft/proposed bodies may also be edited
outside BADR; `validate` checks those edits without modifying status or evidence.

`--editor` requires an absolute executable path resolving outside the target
repository. BADR passes one temporary record path as its only argument, without
a shell, configuration lookup, repository scripts, or Git hooks. It does not
consult `EDITOR`, `VISUAL`, or Git's editor settings. Choose a trusted editor
that waits until saving is complete. The editor runs in a temporary directory,
on a scratch copy under the writer lock. When available, `/dev/tty` receives
editor display output; otherwise output is discarded. JSON stdout and diagnostic
stderr remain clean. A GUI editor can be used if its executable waits for exit;
editor argument strings and shell commands are unsupported.

A successful editor exit submits the scratch copy to structural and complete-change
validation. Exit 130 or SIGINT declares cancellation (exit 2); other unsuccessful
editor exits or launch failures return exit 4. Saving unchanged content returns
`unchanged`, with empty affected IDs/write lists and no proposed commit message.
Invalid input, cancellation, editor failure, and no-ops never change managed
content or evidence. Temporary editor files are removed on normal return.

Editing preserves the allocated ID/path, BRR domain/registry placement, status,
ADR date, applicability, related/replacement links, and ADR lifecycle notes.
Descriptive owners/decision-makers may change; BRR description may change only
in draft/proposed states. Such ordinary edits are not governed lifecycle updates:
they neither require nor fabricate actor/rationale/confirmation, alter an ADR's
latest governed-update date, append transition evidence, approve, or reset.
Governed status, applicability, relationship, or lifecycle-note changes must use
their dedicated commands with authorization and ADR date semantics where applicable.
Approved bodies remain frozen unless an explicitly authorized reset qualifies
under the gates below. Withdrawn/rejected bodies require a successor.

The shared safe writer validates the whole current collection and intended result,
including unrelated records, relationships, evidence, and required history. It
checks expected bytes/file identities and baseline stability before publishing,
then revalidates. Existing unrelated invalidity blocks editing. Valid unrelated
managed/unmanaged changes and staged files are preserved. Only the selected
record is published for ordinary editing; registry and evidence bytes remain
unchanged. Failed publication/final checks use the shared exact completed/pending path report and
retained-lock manual-repair behavior. Individual file replacement is atomic;
arbitrary external editors can still race the final expected-content check.

Human and JSON output report changed IDs, coverage, pending commit durability,
completed/pending writes, and a proposed audit message after successful changes.
`edit` never stages or commits, and successful structural validation is not
human review or authorization.

## Branch-local approval reset

```sh
badr edit 0001 --content /path/to/revised-decision.md \
  --confirm --actor "Human" --rationale "Revise branch-local reasoning"
badr edit BRR-ORD-001 --editor /usr/bin/vi \
  --confirm --actor "Human" --rationale "Revise branch-local policy"
badr edit 0001 --register-external \
  --confirm --actor "Human" --rationale "Register an external reasoning change"
```

An actual approved-body change requires `--confirm`, nonempty `--actor`, and
nonempty `--rationale`. Agents obtain human authorization outside BADR. The
checkout must be a named branch other than the configured baseline, with valid
existing evidence and complete required history. The ID must never have appeared
in baseline history, including earlier drafts/proposals and subsequently deleted
files, and must never have been activated or participated on either side of
completed supersession. Renaming branches, deleting baseline records, withdrawing
successors, or changing baseline configuration cannot unfreeze known durable IDs.
V1 refuses retargeting committed baseline configuration.

Choose exactly one input: `--content`, `--editor`, or `--register-external`.
The last form explicitly registers the body's current on-disk bytes without an
editor or input file. It permits only the target approved body's known digest
mismatch during prevalidation. All other collection, metadata, lifecycle-note,
evidence, and history checks still apply, including unrelated digest mismatches
and invalid committed snapshots. An unchanged body cannot be registered.
`validate` remains read-only and continues reporting unauthorized direct edits.
Ineligible changes must be restored or expressed in a successor.

Reset sets `proposed`, updates an ADR's UTC governed date, and appends an `edit`
event with authorization, actual time, branch, resolved baseline commit, changed
body digest, and prior `approval_ref`. Every earlier receipt is retained exactly.
Another `decide` is required and produces a new approval receipt. Supplied/editor
no-ops retain approval and its active receipt without any new event.

Both record types use complete-change validation, locking, expected-content and
baseline rechecks, atomic individual-file writes, and postvalidation. Publication
order is record then evidence. Success returns `operation: edit`, `result: edited`,
coverage, affected IDs, pending commit durability, and a proposed audit message.
Failure after publication reports completed/pending paths and retains the repair
lock; record and evidence publication is not a crash-atomic transaction.

## Explicit approval

```sh
badr decide BRR-ORD-001 --confirm --reviewed --actor "Human" --rationale "Reviewed cancellation policy"
badr decide 0001 --confirm --reviewed --actor "Human" --rationale "Reviewed storage decision" --json
```

`decide <id>` accepts only proposed BRRs or ADRs. It requires `--confirm`,
nonempty `--actor` and `--rationale`, and `--reviewed`: the caller declares that
a human reviewed the applicable content, relationships, compatibility treatment,
and gaps and resolved them. Agents must obtain human authorization outside BADR.
BADR records this declaration without authenticating the actor or proving an
interview occurred. Descriptive owners/decision-makers and successful validation
provide no authorization. The command works without prompts or editors.

Resolve standalone uppercase `TODO`, `TBD`, and `FIXME` markers in the decision
or rule body before approval. These markers block approval even with `--reviewed`.
Other gaps and semantic adequacy remain the human reviewer's responsibility;
passing a marker/structure check never establishes completeness.

Mark an intended successor BRR with `@successor` on its Feature before approval.
This expresses intent without a completed replacement link or replacement set.
It requires scenarios under its Rule tagged `@compatibility:existing-data` and
`@compatibility:in-flight`. Each must have a nonempty `Then` outcome step and an explicit
`@treatment:grandfather`, `@treatment:migrate`, `@treatment:re-evaluate`, or
`@treatment:reject` tag. One scenario may cover both scopes. For example:

```gherkin
    @compatibility:existing-data @treatment:grandfather
    Scenario: Preserve existing orders
      Given an order was accepted under the prior policy
      When the new policy is introduced
      Then the order retains its original cancellation rights
```

The caller must identify successor intent and review treatment for both scopes;
BADR checks declared structure and records `reviewed: true`, while humans judge
semantic completeness. Approval never infers replacement sets or changes
predecessors, replacement links, or applicability times.

Approval writes `approved` for both types. BRRs stay inactive until a separate
explicit activation. ADRs immediately direct new work in the current checkout,
update their governed date to today's UTC event date, and preserve decision text
and lifecycle notes. Approval does not assess existing-code conformance or
rewrite baseline files. Frontmatter is serialized; exact UTF-8 body bytes,
including comments, whitespace, and LF/CRLF, are preserved for the receipt.

The append-only receipt records actor/rationale, confirmation and resolved-review
declaration, actual UTC time, current named branch, immutable baseline commit,
affected ID/path, proposed-to-approved transition, and SHA-256 with an
`approval_ref` pointing to this receipt. Repeating approval is an error and
creates no event or new digest. Missing evidence, manual status changes, and
unauthorized approved-body edits remain errors, with no automatic repair.

Shared full-collection validation, required history, lock, baseline-race,
expected-content, and manual-repair rules apply. Publication order is record
then evidence, with accurate completed/pending paths on failure. Success returns
`operation: decide`, `result: approved`, affected IDs, baseline/coverage,
pending commit durability, and a proposed audit commit message. The command
never stages or commits; Git durability requires the user's later audit commit.

## Ordinary relationships

```sh
badr link 0001 0002 --confirm --actor "Human" --rationale "Related storage choices"
badr link 0001 BRR-ORD-001 --confirm --actor "Human" --rationale "Supports cancellation"
badr unlink 0001 0002 --confirm --actor "Human" --rationale "Connection no longer relevant"
badr related 0001 --json
```

`link <source-id> <target-id>` and `unlink <source-id> <target-id>` require actual
stable IDs, `--confirm`, and nonempty `--actor` and `--rationale`. Agents obtain
human authorization outside BADR; the command records the caller's declaration.
Permitted fields are ADR `related_adrs` / `related_business_rules` and BRR
`related_architecture_decisions`, selected from source and target types. ADR
identities remain quoted YAML strings. Links are directed; no reciprocal link
is created. Duplicate links and removal of absent links are errors.

These operations preserve identity, status, applicability, completed supersession
edges, frozen body bytes, and existing evidence. They never approve or authorize
replacement. ADR writes update the UTC date and append a dated lifecycle note
with actor/rationale. Governed metadata is serialized while decision/Gherkin
bodies and prior notes retain exact bytes. Full collection/history prevalidation,
intended-state validation, locking, race checks, and shared safe writes apply.
Missing targets in available collections are errors (exit 2); absent supporting
collections leave targets unverified (exit 3), with no publication. Other
unrelated invalidity also blocks mutation.

Successful mutations report `operation: link` / `unlink`, `result: linked` /
`unlinked`, the changed source ID, completed/pending paths, coverage, pending
commit durability, and an audit commit message. Publication writes the source
then appends evidence. A partial failure reports exact paths and retains the
manual-repair lock; files are atomic individually, not as a group. Nothing is
staged or committed automatically.

`related <id>` reads the complete collection before following outgoing ordinary
related references transitively. It includes the starting record, visits each
reachable record once (cycles are permitted), and returns records in stable ID /
path order. JSON uses the existing `records` array with explicitly stored related
fields; human output also prints `source -> target` edges. Supersession edges
are not traversed. Missing/unverified references retain the validation diagnostics
and exit status; traversal does not invent a record for an unavailable target.

## Immediate BRR activation and withdrawal

```sh
badr activate BRR-ORD-001 --confirm --actor "Human" --rationale "Apply reviewed policy" --json
badr deprecate BRR-ORD-001 --confirm --actor "Human" --rationale "Withdraw this policy"
badr retire BRR-ORD-001 --confirm --actor "Human" --rationale "End this policy" --json
```

`activate` accepts only an approved BRR and sets `effective_from` to the actual
UTC operation time. Approval alone remains inactive. `deprecate` and `retire`
accept only effective BRRs, preserve activation time, and set `effective_until`
to the actual withdrawal time. Timestamps are quoted, include seconds and an
explicit UTC timezone, and may include fractional seconds. There are no
scheduling/backdating arguments. These commands reject ADRs and illegal or
repeated transitions. All require `--confirm`, nonempty `--actor` and `--rationale`;
agents obtain human authorization outside BADR, which records a declaration.

Each event preserves the active approval reference, exact frozen body digest,
related/replacement links, and earlier receipts. Effective and withdrawn bodies
remain frozen; activation permanently disqualifies an approval reset. Withdrawn
records cannot reopen. Shared whole-collection, baseline/history, safe-write,
lock, and partial-failure rules apply. Publication order is record then evidence.
Success returns `result: effective`, `deprecated`, or `retired`, the affected ID,
coverage/baseline, pending audit durability, and a proposed audit commit message.
BADR never stages or commits.

Withdrawal selects no replacement and revives no predecessor. Inspection and
successful withdrawal include an advisory diagnostic `policy_gap` for each
deprecated/retired BRR, identifying the record and asking humans to resolve any
policy gap explicitly. This notice does not fail validation, prove that a gap
exists, or assess code conformance. It remains visible when filtering records;
there is no inferred semantic coverage or gap-resolution workflow. Current-effective
and pending-preview selections immediately exclude withdrawn BRRs.

## Reject or deprecate an ADR

```sh
badr reject 0001 --confirm --actor "Human" --rationale "Prefer another option"
badr deprecate 0002 --confirm --actor "Human" --rationale "Direction no longer suitable" \
  --decision-gap "Storage direction awaits a successor" \
  --existing-code-treatment "Retain existing implementation pending an authorized migration"
```

`reject` accepts only proposed ADRs and returns `operation: reject`,
`result: rejected`. For ADRs, `deprecate` accepts only approved records and returns
`operation: deprecate`, `result: deprecated`. Both require explicit `--confirm`,
nonempty `--actor` and `--rationale`. ADR deprecation also requires nonempty
`--decision-gap` and `--existing-code-treatment`; explicitly state `None` when
there is no decision gap. The record ID selects BRR or ADR behavior. BRR deprecation
keeps its existing flags and rejects these ADR-only declarations. Agents obtain human authorization outside BADR;
the command records the declaration without authenticating the actor.

Rejection remains non-authoritative. Deprecation immediately withdraws direction
for new work in this checkout: `list --directing` excludes the ADR and `show`
reports `directs_new_work: false`. Neither command changes existing code,
baseline files, production, or asserts existing-code conformance. Deprecated
ADRs remain eligible for a later explicit supersession through `supersede`.
Rejected/deprecated records cannot reopen through `edit`
or `decide`, and their reasoning stays frozen. IDs and relationships are retained.

Both commands update the quoted governed date to the actual UTC event date,
append a dated lifecycle note with actor/rationale (plus gap/code treatment for
deprecation), and append evidence automatically. Caller text is quoted in notes
so embedded Markdown cannot create lifecycle headings. Exact decision-body bytes,
all earlier note bytes, and receipts are preserved. A Markdown boundary that
prevents appending a real final lifecycle section without changing the decision
body (for example, no terminal newline or an unclosed fence) is refused before
writing. Resolve such boundaries while proposed; approved reasoning needs a
successor if a lifecycle note cannot be appended safely.

These commands use complete-change validation, the shared lock/safe writer,
partial-write/manual-repair reporting, and pending commit durability. Human
output includes completed/pending paths and the proposed audit commit message;
JSON exposes the same report fields. They never stage or commit automatically.

## Explicit complete BRR replacement

Activate previously approved successors and end predecessor applicability:

```sh
badr activate --predecessor BRR-ORD-001 --successor BRR-ORD-002 \
  --successor BRR-ORD-003 --complete --reviewed --confirm \
  --actor "Human" --rationale "These two rules jointly replace cancellation policy"
```

For successors already effective, use `supersede` with the same flags. Repeat
`--predecessor` for consolidation and `--successor` for splitting. Sets must be
nonempty, duplicate-free, disjoint, and contain existing BRRs. Predecessors must
be effective; `activate` successors must be approved, while `supersede` successors
must already be effective. A positional ID remains supported for ordinary
`activate <id>` and cannot be combined with replacement flags.

`--complete` declares that the entire predecessor policy is replaced by the
listed successors jointly. `--reviewed` declares human review of compatibility
for existing data and in-flight behavior. Successors must carry the Feature
tag `@successor` before approval; `decide --reviewed` checks both
`@compatibility:existing-data` and `@compatibility:in-flight` scenarios with a
`treatment:grandfather`, `treatment:migrate`, `treatment:re-evaluate`, or
`treatment:reject` tag and a nonempty `Then` outcome. Prefix each tag with `@`
in Gherkin. Missing treatment requires eligible edit/reset and reapproval, or
a new successor. BADR checks structure and declarations; humans judge completeness.

Both commands retain bodies, prior approval receipts, activation times, and
historical links. They add reciprocal `supersedes`/`superseded_by` links and
reject cycles. Predecessors become `superseded` with an immediate end time;
coordinated activation gives every successor the same immediate start time.
Scheduling/backdating flags are unsupported. Approval alone never withdraws
a predecessor. Completed replacement prevents approval reset on either side.
Later withdrawal exposes gaps without reviving any predecessor. Partial or
intended replacement belongs in reasoning and ordinary related links. ADR groups
use `supersede` for approved successors, or add `--approve-successors --reviewed`
to approve proposed successors as part of the complete replacement. All target
IDs in a group must be ADR IDs or BRR IDs; mixed groups are refused.

JSON uses the existing schema version 1, including all sorted `affected_ids`,
baseline, coverage, pending commit durability, and completed/remaining writes.
Successful `activate` reports `result: "effective"`; successful `supersede`
reports `result: "superseded"`. Human output includes write outcomes and a
proposed audit commit message. Neither command commits managed records.

The shared whole-collection pre/postvalidation, lock, baseline/content rechecks,
and safe individual-file writes apply. These group operations are **not
crash-atomic**. A failure can leave partial replacement; preserve the reported
paths and repair manually using the lock cleanup procedure above. No automatic
rollback or recovery is provided.


## Explicit ADR replacement

```sh
badr supersede --predecessor 0001 --successor 0002 --complete \
  --confirm --actor "Human" --rationale "0002 completely replaces 0001"

# Approve proposed successors and complete replacement as one authorized group.
badr supersede --predecessor 0001 --successor 0002 --successor 0003 --complete \
  --approve-successors --reviewed --confirm --actor "Human" \
  --rationale "Together 0002 and 0003 completely replace 0001"
```

Repeat `--predecessor` for consolidation and `--successor` for splitting. Both
lists are required and must contain distinct, disjoint, actual ADR IDs. Every
listed successor completely replaces every listed predecessor as a group.
`--complete` declares the entire intended replacement set; partial or intended
replacement belongs in reasoning or ordinary related links. BADR records this
human declaration and checks structure; it cannot assess semantic completeness.
Neither ordinary approval nor related links infer replacement.

Predecessors must be approved or deprecated. Successors must be approved, or
proposed when `--approve-successors --reviewed` is supplied. Coordinated approval
uses the same review declaration and unresolved TODO/TBD/FIXME checks as `decide`.
Already-approved successors retain their existing approval references; proposed
successors reference the new group receipt. Rejected, superseded, and deprecated
successors cannot be reopened. `--reviewed` requires `--approve-successors`.
This command supports ADR groups; BRR groups use the same command with BRR
IDs and the compatibility rules above.

The complete intended state is validated before publication, including evidence,
identity, statuses, exact reciprocal edges, and an acyclic replacement graph.
Each record retains previous links, exact decision-body bytes, prior notes, and
receipts. Every affected ADR receives the actual UTC date and a dated lifecycle
note naming both complete sets and actor/rationale. Completed participation on
either side permanently blocks approval reset. Later successor deprecation
retains history and never revives predecessors or changes existing code.

JSON returns `operation: supersede`, `result: superseded`, and all `affected_ids`
in stable ID order. The evidence operation is `supersede` for already-approved
successors or `decide` for coordinated approval, with `replacement` containing
`predecessors`, `successors`, and `complete: true`. Evidence includes every
changed revision, actor, rationale, confirmation, actual UTC time, checkout
branch, baseline commit, and applicable approval references. Human output
includes the shared completed/remaining write paths and proposed audit commit.

Publication uses the shared safe writer and postvalidation, one file at a time,
with evidence appended last. Failure reports exact completed/remaining paths;
once publication starts the repair lock blocks mutations until manual repair.
The group is not crash-atomic and has no automatic rollback or recovery.
The command never stages or commits; pending audit durability remains explicit.
