# Prepared BADR tap (unpublished)

`badr.rb` is a reviewable **unpublished** definition for the proposed
`soheltarir/homebrew-badr` repository, at `Formula/badr.rb`. Neither that
repository nor public release URLs are established by this change. The BADR
repository is currently private; the HTTPS URLs assume separately authorized
public release access. Do not advertise working `brew install` commands until
an unauthenticated installation can actually download the assets.

## Provenance and regeneration

The checked-in 0.1.0 formula uses the four actual archives from successful
[release candidate run 39249567709](https://github.com/soheltarir/badr/actions/runs/39249567709),
commit `6bd70c331ce4b1e9e3baaf7bf0732d29ace6b149`. It is a preparation snapshot,
not a claim that version 0.1.0 has been released. Candidate artifacts expire;
release maintainers must preserve the reviewed bytes or regenerate and review
all checksums after rebuilding. Never substitute different bytes at a published
version URL.

Download all four `badr-TARGET` Actions artifacts into one directory (flattened),
then, from the reviewed checkout:

```sh
python3 packaging/homebrew/test_prepare.py
python3 scripts/prepare-homebrew.py 0.1.0 dist/archives > dist/badr.rb
cmp dist/badr.rb packaging/homebrew/badr.rb
sh scripts/verify-homebrew.sh 0.1.0 dist/archives
```

The generator checks each archive against its named SHA-256 companion before
emitting anything. It requires the full four-target set and always generates
versioned HTTPS release URLs under `vVERSION`. It does not establish trusted
provenance or replace review of the native `VERIFIED` evidence. Python is only
release tooling; it is not an installed BADR dependency. `badr.rb.in` is the
maintained template. Regenerate `badr.rb` for each authorized release using the
same artifacts and version as the direct archive installation instructions.

The verifier refuses an existing BADR keg, creates a unique disposable local
tap, and changes only its test copy's URLs to `file://` archives. It checks
Homebrew's own checksum rejection, real archive extraction, installed version,
`brew test`, and the existing Git-backed BRR/ADR CLI workflow. It then performs
`brew upgrade` to formula revision 1 using the same archive and repeats those
checks. This exercises Homebrew's upgrade mechanism without inventing a second
BADR release; a future two-release upgrade must also be checked with both real
release versions. Cleanup removes the test keg and tap, including on failure.
Run on a disposable host if unrelated Homebrew setup changes are unacceptable.

The native workflow prepares a formula from all four newly built candidates,
then runs these checks on each host. Each Homebrew job uploads the generated
`Formula/badr.rb` and verification log. A configured job is not passing evidence.
See [homebrew-verification.md](homebrew-verification.md) for checks actually exercised locally;
consult the PR's CI results for additional hosts.

## Publication checklist — separate authorization required

1. Obtain explicit release authorization for the repository owner/name,
   public visibility/access to the release assets and tap, tag/version, and
   publication. Obtain the owner's required GitHub credentials outside the
   formula; no token, signing key, or credential belongs in this repository.
2. Review native release and Homebrew logs on all claimed targets. Preserve the
   archive bytes, checksums, source commit, and build information. Publish the
   authorized `vVERSION` release with those exact assets and checksum files.
   Signing/notarization, if requested, needs its own credentials and verification;
   no signing is performed here.
3. Create the authorized `soheltarir/homebrew-badr` repository. Copy the reviewed
   generated formula to `Formula/badr.rb`, commit, and publish. This is a proposed
   ownership/name, not an already performed action. Core acceptance and a bottle
   pipeline are not prerequisites: this formula installs upstream binary archives.
4. On a clean supported host, verify these **post-publication** commands without
   GitHub credentials before documenting the tap as available:

   ```sh
   brew tap soheltarir/badr
   brew install soheltarir/badr/badr
   badr --version
   brew test soheltarir/badr/badr
   ```

5. For a later release, generate/review a formula with its new version and all
   four checksums, verify installation and upgrade from the previous release,
   then publish it in the tap. Users run:

   ```sh
   brew update
   brew upgrade soheltarir/badr/badr
   badr --version
   ```

   Formula revisions are for packaging changes at the same upstream version;
   the local verifier's revision 1 is never copied into the publication formula.

Homebrew's supported hosts are upstream policy, independent of BADR's
[verified runtime baselines and direct archive/source installation](releases.md).
Homebrew availability does not lower those baselines or promise ongoing Intel
macOS Homebrew availability. Linux candidates require glibc 2.35+, even if
Homebrew itself works on an older runtime. Trusted Git remains required for
managed operations; an editor is optional. No Rust, Python, Go, kata, credentials,
or network is needed for ordinary installed management.

## Official documentation checked

Checked 2026-10-19 using Context7 and the official pages:

- [Formula cookbook](https://docs.brew.sh/Formula-Cookbook): nested OS/architecture
  selection, version/checksum stanzas, archive staging, `bin.install`, and tests.
- [Formula API](https://rubydoc.brew.sh/Formula.html): formula DSL.
- [Tap maintenance](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap): repository
  naming, `Formula/` layout, fully qualified installation, updates.
- [Installation](https://docs.brew.sh/Installation): current upstream host support.
- [Manpage](https://docs.brew.sh/Manpage): `fetch`, `install`, `test`, `upgrade`,
  and revision upgrades. Local commands were also checked with Homebrew 6.0.22.
