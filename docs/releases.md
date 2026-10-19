# Releases and installation

Standalone archives and checksums are published in
[GitHub Releases](https://github.com/soheltarir/badr/releases).
Release 0.1.0 is the first public release; signing/notarization is not provided.
[Homebrew tap contents and publication steps](homebrew.md)
are prepared separately, with installation/upgrade verification in the native CI matrix.
The tap remains unpublished. V1 acceptance remains
incomplete until every target has native evidence and the tap is verified.

## Build and runtime requirements

Source builds require Rust **1.85.0 or newer**, Cargo, a native C linker/SDK,
and trusted Git. The release workflow uses 1.85.0 with `Cargo.lock`; dependency
updates must recheck the minimum toolchain. From a reviewed source checkout:

```sh
rustup toolchain install 1.85.0 --profile minimal
cargo +1.85.0 build --locked --release
./target/release/badr --version
mkdir -p "$HOME/.local/bin"
install -m 755 target/release/badr "$HOME/.local/bin/badr"
```

Add `$HOME/.local/bin` to PATH. Building/downloading dependencies requires network
access unless cached/vendor supplied. Ordinary installed operations require
only BADR, trusted Git on PATH, a local Git worktree, and its complete locally
available history. No Python, Go, kata, Rust, credentials, or network is needed
at runtime. No editor launches unless `edit --editor /absolute/executable` is
explicitly requested. The tests use Git's `init -b` (Git 2.28+).

The candidate matrix deliberately chooses conservative runtime floors rather
than claiming Rust's lowest upstream supported systems:

| Target | Candidate runtime floor | Native verification host |
|---|---|---|
| `aarch64-apple-darwin` | macOS 14 / Apple Silicon | `macos-14` |
| `x86_64-apple-darwin` | macOS 15 / Intel | `macos-15-intel` |
| `aarch64-unknown-linux-gnu` | Ubuntu 22.04 ARM64, glibc 2.35 | `ubuntu-22.04-arm` |
| `x86_64-unknown-linux-gnu` | Ubuntu 22.04 x86_64, glibc 2.35 | `ubuntu-22.04` |

These are verification targets, not passing results. Consult the successful
native job and `BUILD-INFO.txt` for actual OS/kernel, architecture, toolchain,
Git, and dynamic libraries. Linux archives use the host's GNU runtime, not
musl or static linking. Other distributions, older kernels/glibc, and lower
macOS deployment targets are unverified. macOS deployment targets are set to
14.0/15.0 respectively. The local ARM64 macOS 27 check cannot establish macOS
14 compatibility; the native matrix must pass before advertising that floor.

`Cargo.lock` pins the complete dependency graph. Actual parser versions are
`yaml_serde` **0.10.7**, `saphyr-parser` **0.2.2**, `yaml-rust2` **0.11.1**,
`gherkin` **0.14.0** (parser feature, no default features), and
`pulldown-cmark` **0.13.4**. These Rust dependencies compile into the executable;
no separately installed YAML/Gherkin interpreter is required. BADR additionally
rejects unsafe tags/duplicate metadata and validates its own record structure.

## Archive preparation and verification

On each native host, from a clean reviewed commit with Rust 1.85.0 selected:

```sh
cargo check --locked --all-targets
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
sh scripts/prepare-release.sh aarch64-apple-darwin
```

Substitute the native target from the matrix. The script refuses a mismatched
host and refuses reuse of an existing staging directory. Move previous `dist/`
results aside before repeating. `dist/` is ignored and is never committed.
On Linux smoke verification uses `sudo unshare --net`; on macOS it uses
`sandbox-exec` with denied networking. These are verification tools, not runtime
dependencies. Failure to create the sandbox fails verification; it is not a pass.

The `Native release candidates` PR/manual Actions workflow builds and tests
each architecture natively, without publishing releases. Its downloadable
artifacts expire after 14 days. Each target artifact contains:

```text
badr-0.1.0-TARGET.tar.gz
badr-0.1.0-TARGET.tar.gz.sha256
badr-0.1.0-TARGET-VERIFIED.txt
```

The archive has a single `badr-0.1.0-TARGET/` root containing `badr`, `LICENSE`,
`THIRD_PARTY_NOTICES.md`, `README.md`, `Cargo.lock`, `BUILD-INFO.txt`, and `docs/`
(including the CLI, JSON schema version 1, and evidence schema version 1).
`BUILD-INFO.txt` records commit and dirty state: a dirty local build is not a
clean release candidate. Archive timestamps are not reproducible-build claims.

The script verifies the checksum, confirms a corrupted copy fails verification,
extracts the original archive, checks `badr --version`, and runs the public CLI
workflow against that extracted binary with networking denied. Only then does
it write the `VERIFIED` marker and allow artifact upload. Candidate checksums
detect corruption; without a separately trusted checksum/signature they do not
authenticate the publisher.

After obtaining a reviewed candidate and its trusted checksum file, install
without elevated privileges (example for ARM64 macOS):

```sh
shasum -a 256 -c badr-0.1.0-aarch64-apple-darwin.tar.gz.sha256
tar -xzf badr-0.1.0-aarch64-apple-darwin.tar.gz
./badr-0.1.0-aarch64-apple-darwin/badr --version
mkdir -p "$HOME/.local/bin"
install -m 755 badr-0.1.0-aarch64-apple-darwin/badr "$HOME/.local/bin/badr"
```

On Linux use `sha256sum -c` and the matching Linux target. Stop if verification
fails. Download archives and matching checksum files from GitHub Releases. Unsigned candidates may require local platform security review;
no signing credentials or notarization have been used.

## Installer

The repository-root `install.sh` installs the archive matching the host's macOS
or GNU/Linux ARM64/x86_64 target. It requires a POSIX shell, `tar`, standard Unix
utilities, and either `sha256sum` or `shasum`. Downloads additionally require
`curl`. Runtime requirements and platform verification limits above still apply;
Linux builds are GNU/glibc builds, not Alpine/musl or Android binaries.

Use a reviewed local candidate now, keeping its checksum file alongside it:

```sh
sh install.sh --archive /path/to/badr-0.1.0-aarch64-apple-darwin.tar.gz
```

The installer defaults to `$HOME/.local/bin`, accepts `BADR_INSTALL_DIR`, and
prints a reminder when that directory is absent from PATH. It does not edit
shell configuration or use `sudo`. Rerunning replaces an existing regular
`badr` binary after checksum and `--version` verification. Directory/symlink
destinations are refused. Downloads and checks happen before replacement, and
the final replacement is a rename on the destination filesystem.

GitHub Releases use `vVERSION` tags and the archive/checksum names above.
The installer resolves the latest stable release by default:

```sh
curl -fsSL https://raw.githubusercontent.com/soheltarir/badr/main/install.sh -o install.sh
# Review the downloaded script before executing it.
sh install.sh
# Or select a specific release and installation directory:
BADR_VERSION=0.1.0 BADR_INSTALL_DIR="$HOME/bin" sh install.sh
```

Downloaded checksums detect corruption but do not independently authenticate
the publisher.
The binary version check executes the downloaded binary, so use trusted releases
or reviewed local candidates. Platform signing/notarization remains a separate
gate. Installer tests use disposable fixture archives and mocked downloads:

```sh
python3 scripts/test-install.py
```

## Acceptance evidence and remaining gates

`tests/runtime.rs` exercises both human and JSON mutation output through the
compiled CLI in temporary repositories: domain/record creation, proposal,
approval, eligible reset/reapproval, activation, related links/traversal,
replacement, retirement/deprecation, validation, deterministic output, schema
version, pending durability, and distinct exits 0/2/3/4. Its processes have
credentials/environment cleared and a PATH containing only trusted Git; the
archive smoke runs the same workflow under OS network denial.

The full accumulated suite supplies the negative and failure-injection checks:

| Acceptance area | Public CLI test files |
|---|---|
| Initialization, grammar, unsafe paths, metadata, JSON/stderr/exit coverage | `cli.rs`, `adr.rs`, `domain.rs` |
| Evidence replay, frozen bytes, full/shallow/baseline history | `evidence.rs` |
| Allocation, concurrent writers, retained/deleted IDs, collisions | `authoring.rs`, `evidence.rs` |
| Reset, no-ops, external registration, reapproval and refusal gates | `editing.rs` |
| Approval, applicability, withdrawal, replacement/splitting, non-revival | `authoring.rs` |
| Directed links, traversal, partial link writes | `relationships.rs` |
| Write-boundary failures, exact completed/pending paths, repair locks | `authoring.rs`, `editing.rs`, `relationships.rs`, `cli.rs` |

Passing existing tests is evidence for these exercised cases, not proof of all
possible histories or failure interleavings. Native CI records the full suite
before preparing each candidate. A cross-compiled executable is insufficient.
Unperformed/failed platform checks remain open release gates; do not describe
V1 or all four platforms as verified from a single local success.

## Approval, durability, and repair limits

Approval records a trusted-local declaration. BADR cannot authenticate a human,
prove review quality, assess implementation conformance, or detect coordinated
rewriting of records, evidence, configuration, and Git history. Keep these
limits in release notes. Ordinary commands never fetch, stage, or commit;
records/evidence remain pending Git durability until the user commits them.

See [the CLI lock cleanup and manual repair procedure](cli.md) before removing
a stale lock or repairing a failed write. Stop writers, preserve the reported
completed/pending paths and all unrelated changes, inspect Git/evidence and
restore a known consistent state, then validate read-only before resuming.
Never discard evidence or refresh a digest to hide an unauthorized change.

Recoverable multi-file transactions remain deferred: each file is replaced
atomically, but power loss or mid-operation failures can leave partial group
changes. A retained repair lock blocks mutations after a known partial failure.
There is no automatic rollback or guaranteed crash recovery. Windows, adoption/
conversion, distributed reservation, and implementation-conformance analysis
are outside this release.

## Authoritative references checked during implementation

- [Rust Apple target requirements and deployment target](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html)
- [Rust target support tiers](https://doc.rust-lang.org/rustc/platform-support.html)
- [GitHub hosted runner platforms](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub workflow artifacts](https://github.com/actions/upload-artifact)
- [yaml_serde 0.10.7](https://docs.rs/yaml_serde/0.10.7/yaml_serde/)
- [Gherkin Rust parser 0.14.0](https://docs.rs/crate/gherkin/0.14.0)

Context7 supplied current Rust and runner documentation. Upstream Rust's lower
OS floor does not establish BADR's runtime floor; native BADR checks control it.

Release profiles explicitly disable stripping because [Rust issue #157750](https://github.com/rust-lang/rust/issues/157750)
reproduces a macOS 27 dyld alignment failure in stripped proc-macro libraries.
This was reproduced locally on Rust 1.85.0 and 1.96.0; explicitly setting
`strip = "none"` allowed the Rust 1.85.0 optimized build to complete.
