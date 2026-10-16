# BADR CLI contract, version 1

BADR implements initialization and read-only BRR/ADR inspection and structural
validation, operation-evidence replay, exact frozen-body checks, and retained
Git revision checks. Record authoring and lifecycle commands remain unavailable.
See the [versioned evidence contract](evidence.md) for the event schema and
coverage limits.

```sh
badr --repo /path/to/existing/repository init --baseline trunk
badr --repo /path/to/existing/repository list
badr --repo /path/to/existing/repository validate --json
```

`--repo` defaults to the current directory. A directory inside a working tree
selects that working tree's root. `--json` and `--repo` may precede or follow
the command. No operation prompts, launches an editor, fetches, runs project
scripts, initializes Git, stages files, or commits.

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
- `operation`: `init`, `list`, `show`, `validate`, or `arguments`.
- `success`: true only for exit 0.
- `result`: `created`/`unchanged` for successful init, `ok` for successful
  inspection, otherwise `invalid`, `incomplete`, or `operational_failure`.
- `exit_code`: the process exit code.
- `affected_ids`: empty for these operations because no record is mutated.
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
  No transaction journal or automatic rollback is provided.

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
