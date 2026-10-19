#!/usr/bin/env python3
"""Render the unpublished tap formula from a complete, checksum-verified release set."""
import argparse
import hashlib
from pathlib import Path
import re
import sys

TARGETS = {
    "darwin_arm": "aarch64-apple-darwin",
    "darwin_intel": "x86_64-apple-darwin",
    "linux_arm": "aarch64-unknown-linux-gnu",
    "linux_intel": "x86_64-unknown-linux-gnu",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", help="Release version, without the v tag prefix")
    parser.add_argument("archives", type=Path, help="Directory containing all four archives and .sha256 files")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", args.version):
        parser.error("expected a numeric release version, optionally with a prerelease suffix")
    formula = (Path(__file__).resolve().parents[1] / "packaging/homebrew/badr.rb.in").read_text()
    formula = formula.replace("@version@", args.version)
    for key, target in TARGETS.items():
        name = f"badr-{args.version}-{target}.tar.gz"
        archive = args.archives / name
        checksum = (args.archives / f"{name}.sha256").read_text().strip()
        match = re.fullmatch(r"([0-9a-fA-F]{64}) [ *]" + re.escape(name), checksum)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if match is None or match[1].lower() != digest:
            raise ValueError(f"checksum mismatch or malformed checksum file: {name}")
        formula = formula.replace(f"@{key}@", digest)
    # Emit only after the complete set passes, so failure cannot publish a partial formula.
    sys.stdout.write(formula)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        sys.exit(str(error))
