# BADR CLI contract, version 1

This milestone implements initialization and empty collection inspection only.
It does not implement record authoring, record parsing, lifecycle/evidence
validation, or historical record checks. A populated collection is never
reported as completely valid or exhaustively listed.

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

`list` and `validate` are read-only and share collection inspection logic.
Absent record directories and evidence, empty record directories, and an empty
evidence file are valid. Other managed files return incomplete coverage until
later milestones implement their validators. Unrelated files outside `.badr/`
are not collected. Baselines resolve once to an immutable local commit ID;
shallow Git history, missing baselines, and any historical managed file other
than configuration prevent complete historical coverage. No snapshot is
checked out and no history content is executed.

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
- `operation`: `init`, `list`, `validate`, or `arguments`.
- `success`: true only for exit 0.
- `result`: `created`/`unchanged` for successful init, `ok` for successful
  inspection, otherwise `invalid`, `incomplete`, or `operational_failure`.
- `exit_code`: the process exit code.
- `affected_ids`, `records`: empty arrays in this milestone. An empty array
  with incomplete coverage does **not** assert that the collection is empty.
- `diagnostics`: ordered objects with `code`, `message`, and managed-relative
  `path` when applicable (otherwise null). Messages are human guidance;
  consumers should branch on exit/coverage and diagnostic codes.
- `baseline_commit`: resolved immutable commit string, or null if unavailable.
- `validation_coverage`: `configuration`, `collection`, `evidence`, `history`,
  each `passed`, `incomplete`, or `not_checked`. Here `passed` means the
  configuration is valid or the corresponding collection/history is empty;
  it does not claim record/evidence validators have been implemented.
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
