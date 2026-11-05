# BADR

BADR helps teams turn business requirements and technical decisions into records
they can review, evolve, and trace in Git. It combines **agent skills** for guided
setup and requirements interviews with a **standalone CLI** for managing records,
their lifecycle, and validation.

- A **Business Rule Record (BRR)** describes required business behavior using
  Gherkin rules and Given/When/Then scenarios. For example: customers may cancel
  an order before fulfilment starts; later cancellation requests are refused.
- An **Architecture Decision Record (ADR)** captures a technical decision, its
  context, options, and consequences. For example: use PostgreSQL to store orders,
  with the reasoning and tradeoffs recorded alongside that choice.

The skills help you discover and write the requirements. The CLI stores records
under `.badr/`, manages explicit state changes, and checks structure and integrity.
People supply the intent, review the content, and authorize approval. You can use
the CLI without an agent, AI service, network access, or credentials once installed.

- [Why BADR?](#why-badr)
- [Use BADR with an agent](#use-badr-with-an-agent)
- [Use the CLI directly](#use-the-cli-directly)
- [After your first draft](#after-your-first-draft)
- [Understand validation](#understand-validation)
- [Reference and development](#reference-and-development)

## Why BADR?

Coding agents make generating and changing code cheap and fast. Reviewing the
result still takes human attention: a large diff can hide assumptions about
business behavior, exceptions, and technical tradeoffs. As generation speeds up,
keeping track of what you asked for and why a choice was made becomes harder.

BADR makes those questions explicit and reviewable. Its requirements skill asks
you to resolve rules, examples, and exceptions, then shows complete draft BRRs
before saving them. This gives you a chance to correct the intended behavior
before it becomes implementation work. ADRs capture technical choices and their
reasoning through the CLI, so those decisions can be reviewed alongside the rules.

The CLI gives agents a consistent way to inspect that direction: which rules are
effective, which decisions are approved, and which changes are still proposals.
Explicit lifecycle operations and retained evidence help you follow how that
direction changes. With the records committed to Git, later sessions can consult
the decisions and their history instead of depending on a previous conversation.

Together, the skills and CLI make intent easier to discuss, agent work easier to
steer, and decisions easier to follow. They give code review an explicit set of
rules and decisions to compare the implementation against. Human review and
testing remain necessary; BADR does not automatically enforce those records in
generated code or prove that the implementation conforms.

## Use BADR with an agent

The repository includes two [Agent Skills](https://agentskills.io/):

| Skill | What it helps you do | Result |
| --- | --- | --- |
| [`badr-init`](skills/badr-init/SKILL.md) | Choose the target Git repository and baseline, initialize BADR, and select which requirement sources the agent may consult. | Validated setup and confirmed source preferences. |
| [`badr-refine-requirements`](skills/badr-refine-requirements/SKILL.md) | Refine new requirements, improve drafts, or work out successor rules through questions, examples, and explicit content review. | Saved, validated **draft BRRs**. |

Both require Git and a working BADR CLI. If the CLI is missing, `badr-init` can
install it after explaining the download and destination and obtaining your
authorization. An existing installation is reused; upgrades require a separate
decision. Persistent agent memory is optional: without it, source preferences
last only for the conversation and must be confirmed in a later session.

These skills currently cover setup and business requirements. They do not provide
an ADR authoring workflow or automatically propose, approve, or activate rules.

### Install the skills

The [CLI installer](#install-the-cli) installs only the executable. Native release
archives do not include the skills; get them from this repository separately.

For a first installation, clone BADR to a separate source directory. Replace the
example paths with your chosen locations:

```sh
git clone https://github.com/soheltarir/badr.git /path/to/badr-source
cd /path/to/your-project
```

Copy the **complete skill folders**, including their `references/` directories,
into your target project's skill directory. If either destination skill already
exists, review it before replacing it; these commands are for a fresh installation.

**Codex** — project skills live in `.agents/skills/`:

```sh
mkdir -p .agents/skills
cp -R /path/to/badr-source/skills/badr-init \
  /path/to/badr-source/skills/badr-refine-requirements .agents/skills/
```

**Claude Code** — project skills live in `.claude/skills/`:

```sh
mkdir -p .claude/skills
cp -R /path/to/badr-source/skills/badr-init \
  /path/to/badr-source/skills/badr-refine-requirements .claude/skills/
```

See the official [Codex skill documentation](https://developers.openai.com/codex/skills/)
and [Claude Code skill documentation](https://code.claude.com/docs/en/skills)
for discovery and other installation scopes. Other Agent Skills-compatible tools
can load the same folders; follow your tool's installation and invocation guidance.

### Create your first draft with an agent

Open your target project in the agent. It must already be a Git working tree with
a local baseline branch that resolves to a commit and sufficient retained history.
You choose the baseline; `main` is a recommendation, not an assumption.

In **Codex**, send these prompts separately, finishing setup before refinement:

```text
$badr-init Set up BADR in this repository. Help me choose the baseline and requirement sources.
```

```text
$badr-refine-requirements Help me define when customers may cancel an order. Interview me about the rules and exceptions, then prepare draft BRRs for my review.
```

In **Claude Code**, use `/badr-init` and `/badr-refine-requirements` with the same
requests instead of the `$` mentions above.

During setup, choose either BADR artefacts only or BADR artefacts plus individually
confirmed specs/docs. During refinement, the agent asks related questions in
rounds, works through scenarios and exceptions, and confirms shared understanding.
It then shows the complete draft content and save operations for your approval.
If a new business domain is needed, it asks separately before registering it.

The result is one or more draft BRRs with IDs, paths, and validation results.
They are not yet approved or effective, and changes remain uncommitted.

## Use the CLI directly

The CLI is a standalone Rust executable. It provides deterministic record and
evidence management; it does not conduct interviews or generate requirements.
You supply the content and explicit authorization for governed transitions.

### Install the CLI

Install the latest release without Rust or elevated privileges:

```sh
curl -fsSL https://raw.githubusercontent.com/soheltarir/badr/main/install.sh | sh
```

The installer selects a macOS or GNU/Linux release for ARM64 or x86_64, verifies
its checksum and binary version, and installs to `~/.local/bin`. Add that directory
to PATH if prompted; `BADR_INSTALL_DIR` selects another destination. See
[release installation](docs/releases.md#installer) for pinned versions, prerequisites,
and options, or browse [GitHub Releases](https://github.com/soheltarir/badr/releases).

```sh
badr --version
badr --help
```

### Create your first draft with the CLI

Use an existing Git working tree with a named checkout branch, a usable local
baseline, and sufficient retained history. The example uses a fresh BADR setup
and a local `main` branch that already has a commit. Choose your actual baseline
explicitly. BADR does not initialize Git or fetch missing history.

**1. Initialize and validate.** Run from the target repository:

```sh
cd /path/to/your-project
badr init --baseline main
badr validate
```

`init` creates `.badr/config.yaml`; record directories are added when needed.
If validation reports missing history or another error, resolve it before
continuing. You can also target a checkout with `badr --repo /path/to/your-project`.

**2. Register a business domain and create a draft.** A domain groups business
rules and gives their IDs a prefix. Replace the sample actor and rationale with
your own authorization details:

```sh
badr domain add ordering --prefix ORD --confirm --actor "Human" \
  --rationale "Establish ordering vocabulary"
badr new brr cancellation --domain ordering --owner Product
```

BADR reports the allocated ID and path. In a fresh setup this is `BRR-ORD-001`.
For an existing setup, reuse an appropriate registered domain and use the actual
allocated ID everywhere below. `Product` is a descriptive owner, not an approver.

**3. Replace the scaffold with your rule.** Save this complete record as
`/path/to/cancellation.feature` outside managed `.badr/` storage, substituting
the actual allocated ID in both frontmatter and the `@brr:` tag:

```gherkin
---
id: BRR-ORD-001
domain: ordering
status: draft
owners: [Product]
---
@brr:BRR-ORD-001 @domain:ordering
Feature: Order cancellation
  Rule: Customers may cancel before fulfilment starts
    Scenario: Cancel before fulfilment
      Given fulfilment has not started
      When the customer requests cancellation
      Then the order is cancelled

    Scenario: Refuse cancellation after fulfilment starts
      Given fulfilment has started
      When the customer requests cancellation
      Then cancellation is refused
```

Apply it through the CLI so the edit is recorded in evidence, then inspect and
validate the saved draft:

```sh
badr edit BRR-ORD-001 --content /path/to/cancellation.feature
badr show BRR-ORD-001
badr validate
```

You now have a validated draft BRR. Review whether the scenarios describe the
actual business policy; structural validity alone cannot establish that.

For technical decisions, `badr new adr storage-choice` creates a proposed ADR
scaffold. Fill in its context, alternatives, decision, and consequences using
`badr edit <allocated-id> --content /path/to/decision.md`. See
[record creation and editing](docs/cli.md#record-creation-and-brr-proposal).

## After your first draft

A BRR follows explicit steps: **draft → proposed → approved → effective**.
Proposal marks it ready for review. Approval records the review declaration;
activation makes the rule applicable. Approval alone does not activate a BRR.
ADRs begin as proposed and, once approved, direct new work; they have no separate
BRR-style activation step.

The CLI also supports withdrawal, eligible approval resets, related links, and
explicit replacement of existing records. Use the [CLI contract](docs/cli.md)
for those commands and their authorization requirements. The requirements skill
ends at drafts; later transitions require separate requests.

Review and commit the managed `.badr/` changes to retain the audit history in
Git. BADR never stages, commits, or publishes them. The [CLI contract](docs/cli.md#governed-domain-registration)
explains pending commit durability and which operational files to leave out.

## Understand validation

`badr validate` checks record structure, relationships, lifecycle consistency,
versioned operation evidence, exact frozen-body revisions, and retained Git
history. Populated collections require consistent evidence; missing evidence is
an integrity error. Unavailable required history is reported as incomplete.

Passing checks does **not** establish business completeness, authenticated human
approval, or that the application implements the recorded behavior. Gherkin
scenarios document examples; BADR does not run them against your application.

Exit codes distinguish success (`0`), invalid input/state/integrity (`2`),
incomplete required coverage (`3`), and operational failures (`4`). Initialization
can write configuration while returning `3`. Inspect the result before continuing.
If a write fails, preserve its report and any retained lock and follow the
[repair procedure](docs/cli.md#governed-domain-registration); do not blindly retry
or delete the lock.

## Reference and development

- [CLI contract](docs/cli.md): commands, lifecycle operations, JSON output,
  exit codes, and lock cleanup.
- [JSON schema](docs/json-schema-v1.json): structured output for scripts;
  add `--json` to operational commands.
- [Evidence contract](docs/evidence.md): retained events, revisions, and coverage.
- [Releases and installation](docs/releases.md): installer options, archives,
  checksums, and native release verification.
- [Specification](docs/specs/badr-spec.md): detailed CLI requirements and boundaries.
- [Third-party notices](THIRD_PARTY_NOTICES.md): licenses and bundled parser data.

To build from source, use Rust 1.85 or newer and Cargo:

```sh
cargo build --locked --release
./target/release/badr --help
```

Development checks:

```sh
cargo check --locked --all-targets
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```
