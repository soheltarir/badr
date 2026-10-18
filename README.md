# BADR

BADR is a standalone Rust command-line tool for managing Business Rule Records
(BRRs) and Architecture Decision Records (ADRs). It supports safe initialization
and governed domain registration, draft BRR/proposed ADR creation, BRR proposal,
ordinary content editing, explicit approval, eligible branch-local approval reset,
read-only inspection, structural
validation, and evidence/frozen-body verification.
Activation, replacement, and the remaining lifecycle commands are planned in subsequent issues.

Build from source with Rust 1.85 or newer and Cargo:

```sh
cargo build --locked --release
./target/release/badr --repo /path/to/repository init --baseline main
./target/release/badr --repo /path/to/repository domain add ordering --prefix ORD \
  --confirm --actor "Human" --rationale "Establish ordering vocabulary"
./target/release/badr --repo /path/to/repository new brr cancellation --domain ordering --owner Product
./target/release/badr --repo /path/to/repository new adr storage-choice
./target/release/badr --repo /path/to/repository edit 0001 --content /path/to/revised-decision.md
./target/release/badr --repo /path/to/repository list --json
./target/release/badr --repo /path/to/repository validate
```

Choose the baseline explicitly; `main` is a recommendation. The target must
already be a Git working tree. Ordinary operations need only the binary and
trusted Git, without network access or credentials.

See the [CLI, JSON schema, exit codes, and lock cleanup contract](docs/cli.md).
Dependencies are pinned in `Cargo.lock`; YAML uses `yaml_serde` 0.10.7 and
Gherkin uses `gherkin` 0.14.0. `saphyr-parser` retains YAML scalar styles
for the required quoted applicability timestamps. Only the exact body after frontmatter reaches the
Gherkin parser. ADR quote validation uses `yaml-rust2` 0.11.1 and Markdown
sections use `pulldown-cmark` 0.13.4. See [third-party notices](THIRD_PARTY_NOTICES.md) for the parser
keyword data used by structural checks.

```sh
./target/release/badr --repo /path/to/repository list --type brr --domain ordering
./target/release/badr --repo /path/to/repository list --effective --json
./target/release/badr --repo /path/to/repository list --pending-preview
./target/release/badr --repo /path/to/repository show BRR-ORD-001 --json
```

Validation replays versioned operation evidence and checks exact SHA-256 body
revisions and retained Git history. See the [evidence contract](docs/evidence.md).
Populated collections require consistent evidence; missing evidence is an
integrity error. Unavailable history is explicitly incomplete. Passing checks
never establishes authenticated approval, semantic adequacy, or code conformance.

Development checks:

```sh
cargo check --locked --all-targets
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Local checks use macOS Apple Silicon. Other platforms, minimum OS versions,
release archives, and Homebrew distribution have not been verified yet.
