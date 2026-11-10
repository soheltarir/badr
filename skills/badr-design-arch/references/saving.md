# Save accepted decisions as proposed ADRs

Used after decision selection in [Decision selection and ADR
conflicts](decisions.md). Use the confirmed absolute repository path, installed
subcommand help, and only CLI-supported operations. Put `--content` in absolute
temporary UTF-8 files outside managed storage. Never run target-repository
scripts or an editor.

Saving needs **Full** mode (validation exit 0, complete required coverage). In a
blocked or limited session, write nothing; return the accepted decisions and the
blocker instead.

## Scope

Each user-accepted decision becomes **exactly one** proposed ADR. Unaccepted
alternatives and unselected candidates produce nothing. This flow never
approves, rejects, deprecates, or supersedes an ADR, never edits an existing
ADR, and never links ADRs. Where the user chose a successor or linked ADR over an
approved one, record the intent in the new ADR's text only; the lifecycle
change needs a separate request.

## Prepare each record

Fix **today's UTC date** once (`date -u +%Y-%m-%d`) and use it as `date`; if
the date changes before writing, refresh it and reconfirm.

```markdown
---
id: "<next-ID>"
status: proposed
date: "<YYYY-MM-DD>"
decision-makers: [<descriptive names or roles>]
related_business_rules: ["BRR-XXX-001"]
---
# <Title>

## Context and Problem Statement
## Decision Drivers
## Considered Options
## Decision Outcome
### Consequences
```

- Exactly one each of **Context and Problem Statement**, **Decision Drivers**,
  **Considered Options**, **Decision Outcome** (level two) and **Consequences**
  (level two, or level three under Decision Outcome); every section has content.
  Decision Outcome states the chosen option and its rationale.
- `status` is `proposed`. `id` and `date` are quoted strings.
- `<next-ID>` is the next free four-digit ID: above every ADR from
  `badr --repo <repo> list --type adr --json` and every ADR ID allocated in
  `.badr/evidence.jsonl` (read only). The CLI is the authority; see Save.
- `related_business_rules` lists only **effective** driver BRRs, quoted. Free-form
  drivers stay in the prose, labelled **not BRR-backed**.
- Leave `supersedes`, `superseded_by`, and `related_adrs` empty or absent; creation
  rejects them. Mention coexisting or replaced ADRs in the prose by ID.
- Record the alternatives and trade-offs the user saw, and the option they chose,
  even when it was not the recommendation.

## Confirm exact content

Show, for every ADR, its complete file with the real date, a lowercase hyphenated
filename slug, and the operation (`new adr --content`). Obtain explicit
confirmation of exact content, date, and operation. The ID is part of the
content: if the CLI rejects it (below), show the corrected content and reconfirm.

Any change afterwards (wording, date, drivers, slug, set of ADRs) invalidates
the confirmation: show the changed content and obtain renewed confirmation
**before any write**. 

## Save

1. Register cleanup (shell `trap` or `finally`) before creating any temporary
   file. Remove only workflow-owned files, on success, failure, cancellation, and
   early return.
2. Validate; require exit 0, complete coverage, and root `.badr.lock` absent:

   ```sh
   badr --repo <repo> validate --json
   ```

3. Per ADR, in order, rechecking the lock before each mutation, write the
   confirmed content to the owned temporary file, then:

   ```sh
   badr --repo <repo> new adr <slug> --content <absolute-temp-file> --json
   badr --repo <repo> show <id> --json
   ```

   Declare driver BRRs in this file: `edit` cannot add relationships later. The
   CLI rejects a mismatched ID **before writing anything**
   (`Supplied content must match the allocated identity`); then recompute the ID,
   show the changed content, and reconfirm. Never use `--register-external` or
   edit an existing ADR. Compare the saved record with the confirmed content:
   `proposed`, same date, drivers, and sections.
4. After the last ADR, validate again:

   ```sh
   badr --repo <repo> validate --json
   ```

## Failures

Inspect `partial_write.completed` on every mutation, even when `success` is
false (creation can reserve an ID without a record). On any failure after the
first write:

- report ADRs **completed** (created, shown), the ADR **in progress**
  (allocated ID, path, `partial_write`), and ADRs **pending** (not started);
- preserve records, evidence, and any lock; delete nothing and never remove a
  lock to release an ID;
- do not retry blindly or continue with the remaining ADRs; stop for repair.

Exit codes: 0 success, 2 invalid input/state, 3 incomplete coverage, 4
operational failure.

## Finish

Report each ADR's ID, path, and status `proposed`; the final validation result
and coverage; that nothing was approved, rejected, deprecated, or superseded;
`pending_commit_durability`, and the changed managed paths from `partial_write.completed`. **Do not stage or
commit.** Later lifecycle actions (approval, rejection, replacement, links)
require separate requests.
