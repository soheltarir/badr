# Homebrew verification evidence

## Local check, 2026-10-19

Host: macOS 27.0.1 (26A434), Darwin 27.0.0, native ARM64; Homebrew 6.0.22.
Homebrew warned that this macOS version is unsupported/pre-release. This result
establishes only the observed local behavior, not upstream Homebrew host support
or BADR's minimum macOS baseline.

Input: all four actual 0.1.0 release candidate archives from
[run 39249567709](https://github.com/soheltarir/badr/actions/runs/39249567709),
source commit `6bd70c331ce4b1e9e3baaf7bf0732d29ace6b149`. The generator verified
all four checksums; only `aarch64-apple-darwin` was executed locally. The
checked-in formula's checksums exactly match these candidates.

Command: `sh scripts/verify-homebrew.sh 0.1.0 dist/archives` (exit 0).
It used an unpublished local tap and `file://` URLs for the actual archives.
Observed results:

- Incorrect formula checksum refused by `brew fetch` with “Formula reports
  different checksum”; expected zero digest differed from actual
  `f37517b98f16b200452ed186d12bb1f2b5a3bb6ede4625731bf86e22381c4e14`.
- `brew install` extracted the actual archive layout and installed 0.1.0;
  the executable returned `badr 0.1.0`.
- `brew test` passed the version check and a Git-backed init/validate workflow.
- `BADR_TEST_BINARY=<installed binary> cargo test --locked --test runtime`
  passed both human and JSON BRR/ADR management workflows, with only trusted
  Git on the managed processes' PATH and credentials/environment cleared.
- `brew upgrade` changed the active keg from `0.1.0` to `0.1.0_1`, followed
  by passing installed version, formula test, and full CLI workflow checks.
- Cleanup removed both test-created kegs and the temporary local tap; absence
  was checked afterwards.

This is a **revision upgrade of the same real archive**, not an upgrade between
two separately released binary versions. There is only one candidate version.
No public HTTPS download, published tap, clean-host public installation, signing,
notarization, or release publication was performed. The Homebrew smoke did not
apply OS network denial; the release candidate smoke separately does so.

Other local checks passed: two generator boundary tests (including corruption,
missing artifacts, invalid version refusal), regeneration comparison against the
checked-in formula, shell syntax, `cargo check --locked --all-targets`, the full
`cargo test --locked` suite, `cargo fmt --check`,
`cargo clippy --locked --all-targets -- -D warnings`, and `git diff --check`.

## Native CI

The PR workflow builds native candidates and checks Homebrew installation and
revision upgrades on macOS ARM64/Intel and Linux ARM64/x86_64. Each Homebrew job
uploads the generated formula and its execution log. At initial preparation,
these new Homebrew jobs have not yet run. Consult the actual PR run results;
configured runners alone are not verification evidence. The earlier successful
release run above verified archives, not Homebrew installation.
