# BADR

BADR is a standalone Rust command-line tool for managing Business Rule Records
(BRRs) and Architecture Decision Records (ADRs). The first milestone supports
safe initialization and empty collection inspection. Record management is
planned in subsequent issues.

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
Dependencies are pinned in `Cargo.lock`; YAML configuration uses `yaml_serde`
0.10.7. No Gherkin/record parser is implemented in this milestone.

Development checks:

```sh
cargo check --locked --all-targets
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Local checks use macOS Apple Silicon. Other platforms, minimum OS versions,
release archives, and Homebrew distribution have not been verified yet.
