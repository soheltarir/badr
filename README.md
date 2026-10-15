# BADR

BADR is a standalone Rust command-line tool for managing Business Rule Records
(BRRs) and Architecture Decision Records (ADRs). It supports safe initialization
and read-only BRR inspection and structural validation. Record authoring,
ADR parsing, and evidence/historical authorization checks are planned in
subsequent issues.

Build from source with Rust 1.85 or newer and Cargo:

```sh
cargo build --locked --release
./target/release/badr --repo /path/to/repository init --baseline main
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
Gherkin parser. See [third-party notices](THIRD_PARTY_NOTICES.md) for the parser
keyword data used by structural checks.

```sh
./target/release/badr --repo /path/to/repository list --type brr --domain ordering
./target/release/badr --repo /path/to/repository list --effective --json
./target/release/badr --repo /path/to/repository list --pending-preview
./target/release/badr --repo /path/to/repository show BRR-ORD-001 --json
```

Populated BRR collections report structural results and exit 3 until evidence
and historical authorization checks are available. Structural validity never
establishes business-rule adequacy, human authorization, or code conformance.

Development checks:

```sh
cargo check --locked --all-targets
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Local checks use macOS Apple Silicon. Other platforms, minimum OS versions,
release archives, and Homebrew distribution have not been verified yet.
