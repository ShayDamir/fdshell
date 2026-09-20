#!/usr/bin/env python3
"""Scan non-test Rust source files by tokei lines-of-code (LoC).

STYLE.md section 2.2 limits non-test Rust source files to at most 90 lines of
code, counted by `tokei`. This tool reports:

  * files in the 80-90 LoC band  -> flagged for future refactoring (2.3);
  * files with more than 90 LoC  -> must be refactored now (2.2 violation).

Run with `--check` to fail (exit code 1) when any non-test Rust file exceeds 90
LoC; that exit-code contract is what the nix flake check gate consumes.

Exit codes: 0 = ok (or nothing over the limit), 1 = files over 90 LoC,
2 = usage/environment error (e.g. `tokei` missing).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

MIN_FLAG = 80  # 80-90 LoC: flag for future refactoring (STYLE.md 2.3)
MAX_OK = 90  # >90 LoC: hard limit (STYLE.md 2.2)

TEST_BASENAMES = {"tests.rs"}  # module unit tests (STYLE.md 2.8)
TEST_DIRNAME = "tests"  # integration-test directory


class ComplexityError(RuntimeError):
    """Raised when the tool cannot run (missing tooling or tokei failure)."""


def normalize_name(root: Path, raw_name: str) -> str:
    """Return a clean repo-relative path from tokei's raw name.

    tokei echoes the path prefix of whatever root it was given, so an absolute
    root yields absolute names. Prefer the path relative to the scan root so the
    report and gate output stay stable regardless of where the script runs.
    """
    try:
        return str(Path(raw_name).relative_to(root))
    except ValueError:
        return raw_name


def scan_rust_files(root: Path) -> dict[str, int]:
    """Return {relative-path: LoC} for every non-test Rust file under root.

    `tokei --streaming json` emits, per file, a placeholder record that carries
    only the file name, followed by the completed record whose nested `stats`
    block holds the real counts. We keep only the completed record, so repeated
    passes over the same file are collapsed and placeholders are ignored.

    Files living under a `tests/` directory or named `tests.rs` are excluded as
    unit/integration tests (STYLE.md 2.8 / AGENTS.md testing section).
    """
    tokei = shutil.which("tokei")
    if tokei is None:
        raise ComplexityError(
            "`tokei` not found on PATH; install it, e.g. `nix-env -iA nixpkgs.tokei`"
        )

    proc = subprocess.run(
        [tokei, "--streaming", "json", str(root)],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        if proc.stderr:
            sys.stderr.write(proc.stderr)
        raise ComplexityError(f"`tokei` exited with status {proc.returncode}")

    counts: dict[str, int] = {}
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        record = json.loads(line)
        if record.get("language") != "Rust":
            continue
        stats = record.get("stats") or {}
        completed = stats.get("stats")
        if completed is None:  # placeholder record, no counts yet
            continue
        raw_name = stats.get("name")
        if not raw_name or not raw_name.endswith(".rs"):
            continue
        name = normalize_name(root, raw_name)
        parts = Path(name).parts
        if parts[-1] in TEST_BASENAMES or TEST_DIRNAME in parts[:-1]:
            continue
        counts[name] = completed.get("code", 0)
    return counts


def grouped(
    counts: dict[str, int],
) -> tuple[list[tuple[int, str]], list[tuple[int, str]]]:
    """Split into (flagged 80-90, over-90), each sorted by LoC desc then path."""
    flagged = sorted(
        ((c, p) for p, c in counts.items() if MIN_FLAG <= c <= MAX_OK),
        key=lambda item: (-item[0], item[1]),
    )
    over = sorted(
        ((c, p) for p, c in counts.items() if c > MAX_OK),
        key=lambda item: (-item[0], item[1]),
    )
    return flagged, over


def print_report(root: Path, counts: dict[str, int], flagged, over) -> None:
    print(f"FD complexity (tokei LoC) on {root}")
    print(f"Scanned {len(counts)} non-test Rust file(s).")
    print()
    print(f"Files {MIN_FLAG}-{MAX_OK} LoC (flag for future refactoring, STYLE.md 2.3):")
    if flagged:
        for code, path in flagged:
            print(f"  {code:>4}  {path}")
    else:
        print("  none")
    print(f"({len(flagged)} file(s))")
    print()
    print(f"Files >{MAX_OK} LoC (must refactor now, STYLE.md 2.2):")
    if over:
        for code, path in over:
            print(f"  {code:>4}  {path}")
    else:
        print("  none")
    print(f"({len(over)} file(s))")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Scan non-test Rust files by tokei lines-of-code."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=None,
        help="directory to scan (default: the repository root)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 if any non-test Rust file exceeds 90 LoC, else 0",
    )
    args = parser.parse_args(argv)

    root = Path(args.root) if args.root is not None else Path(__file__).resolve().parent.parent
    if not root.is_dir():
        print(f"error: scan root '{root}' is not a directory", file=sys.stderr)
        return 2

    try:
        counts = scan_rust_files(root)
    except ComplexityError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2

    flagged, over = grouped(counts)
    print_report(root, counts, flagged, over)

    if args.check:
        if over:
            print(
                f"error: {len(over)} non-test Rust file(s) exceed {MAX_OK} LoC",
                file=sys.stderr,
            )
            return 1
        return 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
