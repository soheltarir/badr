# BRR authoring and saving

Use the confirmed absolute repository path, installed subcommand help, and literal
user arguments. Put `--content` in absolute temporary UTF-8 files outside managed
storage. Register cleanup at creation via trap/`finally` for every exit. Never run
target-repository scripts or an editor.

## Prepare the rule

A BRR is YAML frontmatter plus Gherkin containing one Feature, exactly one named
Rule, and at least one named scenario or populated outline. Use placeholders only
for pre-save review:

```gherkin
---
id: <CLI-allocated-ID>
domain: ordering
status: draft
owners: [Product]
---
@brr:<CLI-allocated-ID> @domain:ordering
Feature: Order cancellation
  Rule: Customers may cancel before fulfilment
    Scenario: Refuse after fulfilment starts
      Given fulfilment has started
      When the customer requests cancellation
      Then cancellation is refused
```

Use reviewed behavior, domain, and owners. Drafts have no applicability times;
creation starts with empty relationship lists. Owners are descriptive. Optional
metadata: `description` and `related_architecture_decisions` with quoted numeric
ADR IDs. Links and completed replacement edges require later, separately
authorized operations.

A successor adds `@successor` beside Feature ID/domain tags and names intended
predecessors plus split/consolidation scope in the Feature description. This
states intent, not completed replacement. Include observable scenarios for
existing data and in-flight operations. Each applicable scope needs its tag and a
user-chosen treatment:

```gherkin
    @compatibility:existing-data @treatment:grandfather
    Scenario: Preserve accepted orders
      Given an order was accepted under BRR-ORD-001
      When the successor policy is introduced
      Then its cancellation rights remain unchanged
```

Treatments are `@treatment:grandfather`, `@treatment:migrate`,
`@treatment:re-evaluate`, or `@treatment:reject`; never default one. Each
compatibility scenario needs a nonempty `Then`. Resolve inapplicable scopes with
the user and an observable boundary scenario. Agreed unresolved items may remain,
but block proposal/approval readiness. CLI structure checks do not replace
semantic review.

## Register an authorized domain

Reuse registered domains and check registry slug/prefix collisions. For a new
one, use the separately confirmed slug, uppercase prefix, actor, and rationale:

```sh
badr --repo <repo> domain add <slug> --prefix <PREFIX> \
  --confirm --actor <authorized-human> --rationale <purpose> --json
```

On failure, inspect completed writes and stop. Vocabulary/glossary changes need
separate authorization.

## Save through BADR

Revalidate and require root `.badr.lock` absent immediately before every mutation.
For a new rule, let BADR allocate the ID; never predict it:

```sh
badr --repo <repo> new brr <title-slug> --domain <domain> \
  --owner <confirmed-owner> --json
```

Use a lowercase hyphenated slug and repeat `--owner` as needed. From a successful
result, read `affected_ids`, then `show` the record. Substitute the allocated ID
and matching Feature tag into reviewed content without changing behavior; encode
owner text as valid YAML. The created scaffold is already a registered draft.

For an existing confirmed draft, reread full content/status and preserve identity,
domain, status, and relationships. If it changed since review, reconcile with the
user. Do not use `--register-external` or approval-reset flags. Write the reviewed
record to the owned temporary file, then:

```sh
badr --repo <repo> edit <actual-draft-id> --content <absolute-temp-file> --json
badr --repo <repo> show <actual-draft-id> --json
badr --repo <repo> validate --json
```

Compare saved content/metadata with the reviewed version after ID substitution.
Every affected record must remain `draft`; predecessors, applicability, and
replacement state must be unchanged. Cleanup removes all workflow-owned temporary
files. Preserve managed records, registry, and evidence.

## Interpret results

Exit 0 plus complete required coverage means structural/integrity success. Exit 2
means invalid input/state/integrity, 3 incomplete coverage, and 4 operational
failure. Read diagnostics and `validation_coverage`; missing history/collections
never permits bypass.

For every mutation inspect `partial_write.completed`, even when `success` is false.
Multi-file writes are not crash-atomic: creation can reserve an ID without a
record, or failed editing can leave a scaffold. Derive pending work from the
confirmed plan, completed paths, affected IDs, diagnostics, and managed state.
Report complete rules, scaffolds/partial writes, pending operations, and retained
locks. Preserve allocation/evidence and stop for repair. Never delete a record to
release its ID, invent evidence, remove a repair lock, or blindly retry.

Report `pending_commit_durability` and changed managed paths without staging or
committing. Validation proves neither approval, business completeness, nor
implementation conformance. Later lifecycle actions require separate requests.
