---
name: badr-init
description: Initialize BADR in an existing Git repository and select repository-scoped requirement sources. Use when setting up BADR or changing which documents its requirements interviews may consult.
---

# Initialize BADR

Set up the selected repository without replacing configuration or creating rules.
This skill is self-contained; another installed skill is not required.
Requires Git and a working BADR CLI; persistent memory is optional.

## 1. Locate the repository and CLI

Resolve the intended checkout with `git -C <candidate> rev-parse --show-toplevel`.
If the target is unclear, suggest the current Git root and ask the user to choose.
Inspect working-tree changes and preserve them. Require an existing Git worktree;
report missing Git or a non-repository target without initializing Git.

Reuse a working `badr` executable: check `badr --version`, `badr --help`, and
the needed subcommand help. Use its absolute executable path consistently if it
is outside PATH. An older or broken installation needs a separate user decision;
do not automatically replace or upgrade it.

If BADR is absent, read [release installation](references/install.md) and follow
that branch. Finish with a working binary or a precise prerequisite/platform
blocker. Package-manager installation and source builds are outside this workflow.

## 2. Confirm the baseline and initialize

Read existing `.badr/config.yaml` if present. Discover local branches, for example
with `git -C <repo> for-each-ref --format='%(refname:short)' refs/heads/`, and verify
that a suggested baseline resolves to a commit. Prefer an existing configured
baseline; otherwise recommend an existing `main`, `master`, or `trunk`, guided by
the repository. Ask the user to confirm the target repository and baseline before
initializing. A remote-tracking ref is not a local baseline branch.

Run commands with the confirmed absolute repository path, quoting paths and user
text as literal arguments:

```sh
badr --repo <repo> init --baseline <confirmed-local-branch> --json
badr --repo <repo> validate --json
```

Matching configuration is preserved byte for byte. If configuration conflicts or
is malformed, stop and report it; do not overwrite, retarget, or remove it. Init
creates only configuration, not domain scaffolds, rules, or agent instructions.
If the repository has no usable local baseline, ask for its intended name before
init and explain that init may write configuration but return incomplete coverage.
Establishing branches/commits or fetching history needs a separate request.

Check process exit, `success`, `validation_coverage`, diagnostics, and
`partial_write.completed` / `not_completed`. Exit 0 means required checks passed;
2 means invalid arguments/state/integrity, 3 incomplete coverage, and 4 operational
failure. Init can write configuration and still return 3. Report that state
accurately; it does not permit governed record writes. After partial writes or a
retained `.badr.lock`, stop for repair, preserving the report and existing files.
Never delete a lock or rerun mutations blindly. Validation proves neither human
approval nor business completeness.

## 3. Select requirement sources

Explicitly ask which content future refinement should consult:

- **BADR artefacts only**: current `.badr/` records, relationships, registry,
  domain glossaries, configuration, and integrity results.
- **BADR artefacts plus existing specs/docs**: the above plus individually
  confirmed document paths in this checkout.

For the second option, discover likely paths by filename first (for example
`docs/specs/`, `specs/`, `docs/requirements/`, and `README.md`); suggest concrete
files and ask the user to confirm the selection before reading their content.
Exclude generated files, dependencies, archives, and other checkouts. Confirm
the files within any selected directory rather than treating it as a future
wildcard. An empty selection requires clarification. Do not silently add ADRs or
docs outside `.badr/` to the first option. Host instructions still govern agent
behavior; that does not make their linked specs requirement sources.

Use current-checkout bytes, including local edits. The baseline is an integrity
reference, not a substitute source of requirement content. BADR may inspect Git
history internally for integrity; this does not broaden the interview source set.

## 4. Remember and hand off

Store the confirmed preference through the host's supported persistent-memory
facility, scoped to this checkout's canonical absolute Git root. Record:

```text
repository: <canonical absolute Git root>
source_mode: badr-only | badr-plus-docs
document_paths: <explicit repository-relative files; empty for badr-only>
confirmed_at: <date>
```

The user's source-selection confirmation authorizes remembering this preference.
Respect host memory permissions; if storage is absent, forbidden, or fails, keep
the same preference in this conversation and explain that another session must
ask again. Do not create a memory substitute in the repository, modify BADR
configuration, or copy preferences to another clone/worktree automatically.
Record preferences only, not document contents or interview answers.

Report the executable/version, repository, baseline, initialization/validation
outcome, selected paths, and whether preferences are persistent or session-only.
Leave configuration changes uncommitted. Proposal, approval, activation,
replacement, staging, commits, and publishing each need separate requests.
