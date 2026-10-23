# BRR authoring and saving

Check the installed subcommand help before running these commands. Use the
confirmed absolute repository path and quote user strings as literal arguments.
`--content` paths should be absolute temporary files outside managed storage.
Do not invoke an editor or execute scripts from the target repository.

## Prepare a complete rule

Every BRR has YAML frontmatter and a Gherkin body: one Feature, exactly one named
Rule, and at least one named scenario (or populated scenario outline) under that
Rule. The example below uses a new-ID placeholder only for pre-save review:

```gherkin
---
id: <CLI-allocated-ID>
domain: ordering
status: draft
owners:
  - Product
---
@brr:<CLI-allocated-ID>
@domain:ordering
Feature: Order cancellation
  Rule: Customers may cancel before fulfilment begins
    Scenario: Cancel before fulfilment
      Given a confirmed order has not entered fulfilment
      When its authorised customer requests cancellation
      Then the order becomes cancelled
    Scenario: Refuse cancellation after fulfilment begins
      Given a confirmed order has entered fulfilment
      When its authorised customer requests cancellation
      Then cancellation is refused and the order continues fulfilment
```

Use the confirmed domain, owners, and actual reviewed behavior. Drafts have no
applicability timestamps. Leave relationship lists empty on creation. Owners are
descriptive. Optional metadata includes `description` and
`related_architecture_decisions` (quoted numeric ADR IDs); any ordinary link
mutation needs a separate request. Completed `supersedes`/`superseded_by` edges
are written by later authorized replacement operations, never by draft authoring.

For a successor, put `@successor` on the Feature alongside the ID/domain tags.
Identify intended predecessor IDs in the Feature description before the Rule,
including split/consolidation scope. This is intent, not completed replacement.
Include observable scenarios for both existing data and in-flight operations.
Each scope needs its compatibility tag and an explicit supported treatment:

```gherkin
    @compatibility:existing-data @treatment:grandfather
    Scenario: Preserve accepted orders
      Given an order was accepted under BRR-ORD-001
      When the successor policy is introduced
      Then that order retains its original cancellation rights

    @compatibility:in-flight @treatment:re-evaluate
    Scenario: Re-evaluate a pending cancellation
      Given a cancellation request is pending under BRR-ORD-001
      When the successor policy is introduced
      Then the request is evaluated under the new eligibility rule
```

These treatments are examples for the user to decide, not defaults. Supported
tags are `@treatment:grandfather`, `@treatment:migrate`,
`@treatment:re-evaluate`, and `@treatment:reject`. Each compatibility scenario must
have a nonempty `Then` outcome. If a scope is inapplicable, explicitly resolve
why with the user and express the boundary with an observable scenario. A draft
may retain explicitly agreed unresolved items, but must not be called ready for
proposal/approval; unresolved compatibility remains a stated blocker. Human
review determines semantic adequacy beyond the CLI's structural checks.

## Register a domain only when separately authorized

Read the current registry to avoid slug/prefix collisions. Reuse existing domains
without re-registering. For a new domain, use the confirmed slug, uppercase
prefix, human actor, and purpose/rationale:

```sh
badr --repo <repo> domain add <slug> --prefix <PREFIX> \
  --confirm --actor <authorized-human> --rationale <confirmed-purpose> --json
```

Do not populate the resulting glossary or change existing vocabulary without
separate authorization. Stop if registration fails; inspect completed writes.

## Save through BADR

Revalidate the collection before writing. For a new rule, avoid guessing the next
number or using supplied content with a predicted identity. Create the scaffold
after confirmation, with a lowercase hyphen-separated filename slug and the
confirmed owners (repeat `--owner` for multiple owners):

```sh
badr --repo <repo> new brr <title-slug> --domain <domain> \
  --owner <confirmed-owner> --json
```

Read `affected_ids` from the successful creation result, then `show` the actual
record. BADR allocates across current state, evidence, and locally referenced
history. Keep the allocated ID and matching Feature tag; substitute them for
the two review placeholders without changing the reviewed behavior. Serialize
valid YAML for owner text rather than concatenating unsafe user strings.
The scaffold is a registered draft, not the completed reviewed business rule.

For an existing draft the user confirmed reusing, read its full content/status
again and preserve its identity, domain, status, and existing relationships.
If it changed after review, stop and reconcile with the user before writing.
Do not use `--register-external` or approval-reset flags. Save the complete
reviewed record to an external temporary UTF-8 file and run:

```sh
badr --repo <repo> edit <actual-draft-id> --content <absolute-temp-file> --json
badr --repo <repo> show <actual-draft-id> --json
badr --repo <repo> validate --json
```

Compare saved content and metadata to the reviewed content with allocated IDs
substituted. Confirm that every affected record remains `draft`, predecessors
are unchanged, and no applicability/replacement transition occurred. Clean up
only your own temporary content files. Keep the saved draft and its evidence;
BADR owns managed identities, registry, and evidence bytes.

## Interpret failures and finish

Exit 0 with complete required coverage means structural/integrity checks passed.
Exit 2 is invalid input/state/integrity, 3 incomplete coverage, and 4 operational
failure. Read diagnostics and `validation_coverage`; missing history or a
supporting collection is not permission to bypass checks.

For every mutation inspect `partial_write.completed` and `not_completed`, even
when `success` is false. Multi-file writes are not crash-atomic. A failed creation
can reserve an ID before the record exists; a successful creation followed by a
failed edit can leave the original scaffold. Report which rules are fully saved,
which are scaffolds or partial writes, pending operations, and any retained lock.
Preserve the allocation/evidence and stop for repair. Never delete a registered
record to release its ID, invent evidence, remove a repair lock, or retry the
batch blindly. Read-only diagnostics remain available for repair assessment.

Report `pending_commit_durability` and changed managed paths without staging or
committing. Validation cannot prove human approval, business completeness, or
implementation conformance. Saving ends here; later lifecycle actions need
separate requests.
