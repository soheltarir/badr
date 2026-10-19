#!/bin/sh
# Prepare one native candidate; never publish a release or create a tag.
set -eu
cd "$(dirname "$0")/.."
target=${1:?Usage: sh scripts/prepare-release.sh TARGET}
case "$(uname -s):$(uname -m):$target" in
  Darwin:arm64:aarch64-apple-darwin|Darwin:x86_64:x86_64-apple-darwin|Linux:aarch64:aarch64-unknown-linux-gnu|Linux:x86_64:x86_64-unknown-linux-gnu) ;;
  *) echo 'Native target does not match this host' >&2; exit 1 ;;
esac
case "$target" in
  aarch64-apple-darwin) export MACOSX_DEPLOYMENT_TARGET=14.0 ;;
  x86_64-apple-darwin) export MACOSX_DEPLOYMENT_TARGET=15.0 ;;
esac
cargo build --locked --release --target "$target"
version=$(cargo pkgid | sed 's/.*@//')
name=badr-$version-$target
mkdir -p dist
# Exclusive directory creation refuses reuse of stale candidate contents.
mkdir "dist/$name"
cp "target/$target/release/badr" "dist/$name/badr"
cp LICENSE THIRD_PARTY_NOTICES.md README.md Cargo.lock "dist/$name/"
cp -R docs "dist/$name/docs"
{
  printf 'Candidate: %s\nCommit: %s\n' "$name" "$(git rev-parse HEAD)"
  git status --porcelain --untracked-files=normal
  uname -a
  rustc -vV
  cargo --version
  git --version
  case "$target" in
    *apple*) sw_vers; otool -L "dist/$name/badr"; vtool -show-build "dist/$name/badr" ;;
    *linux*) cat /etc/os-release; getconf GNU_LIBC_VERSION; ldd "dist/$name/badr" ;;
  esac
} > "dist/$name/BUILD-INFO.txt"
tar -czf "dist/$name.tar.gz" -C dist "$name"
cd dist
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$name.tar.gz" > "$name.tar.gz.sha256"
  sha256sum -c "$name.tar.gz.sha256"
else
  shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256"
  shasum -a 256 -c "$name.tar.gz.sha256"
fi
# Check corruption refusal against a disposable copy, preserving the candidate.
mkdir "$name-check"
cp "$name.tar.gz" "$name.tar.gz.sha256" "$name-check/"
printf '\ncorrupt\n' >> "$name-check/$name.tar.gz"
if (cd "$name-check" && if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "$name.tar.gz.sha256"
else shasum -a 256 -c "$name.tar.gz.sha256"; fi); then
  echo 'Corrupted archive unexpectedly passed checksum' >&2; exit 1
fi
mkdir "$name-check/extracted"
tar -xzf "$name.tar.gz" -C "$name-check/extracted"
binary="$PWD/$name-check/extracted/$name/badr"
test "$("$binary" --version)" = "badr $version"
cd ..
case "$target" in
  *apple*) runner='["sandbox-exec", "-p", "(version 1)(allow default)(deny network*)"]' ;;
  *linux*) runner='["sudo", "--preserve-env=BADR_TEST_BINARY", "unshare", "--net", "--"]' ;;
esac
# Cargo selects the executable from this build, even with stale artifacts present.
BADR_TEST_BINARY="$binary" cargo test --locked --offline --release --target "$target" \
  --config "target.$target.runner=$runner" --test runtime -- --nocapture
printf 'Extracted version, checksum rejection, and network-denied CLI smoke passed.\n' \
  > "dist/$name-VERIFIED.txt"
