# Issue tracker: GitHub

Track work in https://github.com/soheltarir/badr/issues using the `gh` CLI.
Pass `--repo soheltarir/badr` explicitly on issue and label commands.
Keep specifications in their authoritative repository locations; BADR's
current specification remains `docs/specs/badr-spec.md`.

## Publishing and fetching

- Create an issue with an explicit title and `--body-file <file>` for
  multiline content. Apply the configured triage label; approved
  implementation tickets use `ready-for-agent`.
- Read a ticket with `gh issue view <number> --repo soheltarir/badr
  --json number,title,body,labels,comments`.
- List tickets with `gh issue list --repo soheltarir/badr --state open
  --limit 100 --json number,title,body,labels,assignees,url`, using filters
  as needed and fetching further pages when the result limit is reached.
- Add comments with `gh issue comment` and labels with `gh issue edit`.
  Use body files for multiline comments and body updates.
- Resolve completed work by commenting with the outcome and closing
  the issue. Do not close or alter parent issues unless requested.
- Issue numbers and URLs refer to GitHub, not former local ticket numbers.

## Dependencies

Use native issue blocking links as the canonical representation.
Create with `gh issue create --blocked-by <numbers>` or update with
`gh issue edit <number> --add-blocked-by <blocker-numbers>`.
Publish blockers first and use their actual GitHub numbers.

For API operations, list blockers at
`repos/soheltarir/badr/issues/<number>/dependencies/blocked_by`.
The POST operation accepts the blocker's numeric database `issue_id`,
not its issue number or GraphQL node ID.
If native dependencies are unavailable, record `Blocked by: #N, #M`
in the issue body and explicitly report that fallback.
A ready-for-agent issue can start only when all its blockers are closed.

## Existing local tickets

When migrated, retain the original local ticket files as historical
snapshots and record their GitHub URLs in `.scratch/badr/github-issues.md`.
GitHub then owns their current state; snapshot Status fields are historical.
If given an old local ticket reference, follow that mapping rather than
creating or working a duplicate local ticket.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## Wayfinding

When using wayfinder, use a map issue labeled `wayfinder:map`, with child
issues linked through native sub-issues. Use `--parent` on creation or
`--add-sub-issue` on the parent. If sub-issues are unavailable, put a task
list in the map and `Part of #<map>` in each child.

Use `wayfinder:<type>` labels for research, prototype, grilling, or task
children, creating those labels only when that workflow needs them.
Select the first open, unblocked, unassigned child in map order.
Claim it by assigning yourself before work. Resolve by commenting with
the answer, closing the child, and appending its gist/link to the map's
Decisions-so-far. Assignment and closed state replace local claimed/resolved.
