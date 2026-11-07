#!/usr/bin/env python3
"""Run cargo-mutants from per-file `// mutants:` comments.

A mutated source file declares its test map on one line:

    // mutants: -C --lib -- -E 'test(/^parse::case_block::/)'

The payload is exactly the cargo-mutants arguments: every `-C` pair is a
cargo/nextest build target, and the arguments after `--` are passed to
nextest. This keeps the mapping copy-pasteable:

    cargo mutants -f <file> --test-tool nextest -C --lib -- \
        -E 'test(/^parse::case_block::/)'

The tool validates the comments with `cargo nextest list`, then groups files
by cargo-target set and runs one cargo-mutants invocation per group. Grouping
is important: a per-file invocation pays a clean build per worker, which
dominates small files.

Equivalent mutants that no test can kill live in a separate allowlist file
(default: `tools/mutants_allowlist.toml`). An allowlist entry names the source
file and the exact cargo-mutants description; the gate reports it but does not
fail. Keep the reason next to every entry.

Commands:

    tools/mutants_gate.py --check <file>...
    tools/mutants_gate.py --check --changed [BASE]
    tools/mutants_gate.py --check --all
    tools/mutants_gate.py --run <file>...
    tools/mutants_gate.py --run --changed [BASE]

Exit codes: 0 = clean, 1 = validation or mutation failure, 2 = usage or
environment error (missing tooling, bad root, no files selected).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import NamedTuple

COMMENT_RE = re.compile(r"^[ \t]*//[ \t]*mutants:[ \t]*(.*?)[ \t]*$")
BINARY_RE = re.compile(r"binary\(([^)]+)\)")
TEST_RE = re.compile(r"test\(")
MISSED_RE = re.compile(r"^(?P<file>.+):(?P<line>\d+):(?P<column>\d+): (?P<mutant>.*)$")
DEFAULT_ALLOWLIST = "tools/mutants_allowlist.toml"


class GateError(RuntimeError):
    """Raised when a comment or the environment makes the gate impossible."""


class MissingComment(GateError):
    """Raised when a mutated file has no `// mutants:` comment."""


class AllowEntry(NamedTuple):
    path: Path  # repo-relative source file
    line: int
    column: int
    mutant: str  # exact cargo-mutants description
    reason: str


class Mapping(NamedTuple):
    path: Path  # repo-relative source file
    package: str
    cargo_args: tuple[str, ...]
    filter_expr: str


def run(
    argv: list[str],
    root: Path,
    *,
    env: dict[str, str] | None = None,
    capture: bool = True,
) -> subprocess.CompletedProcess[str]:
    """Run a command from `root`, raising GateError on missing executables."""
    try:
        return subprocess.run(
            argv,
            cwd=root,
            env=env,
            capture_output=capture,
            text=True,
        )
    except FileNotFoundError as err:
        raise GateError(f"`{argv[0]}` not found on PATH") from err


def cargo_metadata(root: Path) -> list[tuple[Path, str]]:
    """Return (package directory, package name) pairs, longest path first."""
    proc = run(["cargo", "metadata", "--no-deps", "--format-version", "1"], root)
    if proc.returncode != 0:
        raise GateError(f"`cargo metadata` failed:\n{proc.stderr.strip()}")
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as err:
        raise GateError(f"`cargo metadata` returned invalid JSON: {err}") from err
    packages = []
    for package in data.get("packages", []):
        manifest = package.get("manifest_path")
        name = package.get("name")
        if manifest and name:
            packages.append((Path(manifest).parent.resolve(), name))
    packages.sort(key=lambda item: len(item[0].parts), reverse=True)
    return packages


def package_for(path: Path, root: Path, packages: list[tuple[Path, str]]) -> str:
    """Return the package containing `path`, or fail if there is no match."""
    absolute = (root / path).resolve()
    for directory, name in packages:
        if absolute.is_relative_to(directory):
            return name
    raise GateError(f"cannot determine the cargo package for `{path}`")


def mutated_files(root: Path) -> set[Path]:
    """Return every source file cargo-mutants would mutate."""
    proc = run(["cargo", "mutants", "--list"], root)
    if proc.returncode != 0:
        raise GateError(f"`cargo mutants --list` failed:\n{proc.stderr.strip()}")
    files: set[Path] = set()
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        path = line.split(":", 1)[0]
        if path.endswith(".rs"):
            files.add(Path(path))
    return files


def changed_files(root: Path, base: str) -> set[Path]:
    """Return modified, staged, and untracked `*.rs` files since `base`."""
    commands = [
        ["git", "diff", "--name-only", "--diff-filter=ACMR", f"{base}...HEAD"],
        ["git", "diff", "--name-only", "--diff-filter=ACMR"],
        ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMR"],
        ["git", "ls-files", "--others", "--exclude-standard"],
    ]
    files: set[Path] = set()
    for argv in commands:
        proc = run(argv, root)
        if proc.returncode != 0:
            raise GateError(
                f"`{shlex.join(argv)}` failed:\n{proc.stderr.strip()}"
            )
        for line in proc.stdout.splitlines():
            path = Path(line.strip())
            if path.suffix == ".rs":
                files.add(path)
    return files


def parse_comment(path: Path, root: Path, package: str) -> Mapping:
    """Parse and syntactically validate one `// mutants:` comment."""
    source = root / path
    if not source.is_file():
        raise GateError(f"`{path}` does not exist")
    matches = [
        (line_number, match.group(1))
        for line_number, line in enumerate(
            source.read_text(encoding="utf-8").splitlines(), 1
        )
        if (match := COMMENT_RE.match(line))
    ]
    if not matches:
        raise MissingComment(f"`{path}` has no `// mutants:` comment")
    if len(matches) > 1:
        lines = ", ".join(str(number) for number, _ in matches)
        raise GateError(f"`{path}` has multiple `// mutants:` comments (lines {lines})")

    payload = matches[0][1]
    try:
        tokens = shlex.split(payload)
    except ValueError as err:
        raise GateError(f"`{path}`: cannot parse comment: {err}") from err
    if tokens.count("--") != 1:
        raise GateError(f"`{path}`: comment must contain exactly one `--`")
    separator = tokens.index("--")
    before = tokens[:separator]
    after = tokens[separator + 1 :]

    if len(before) % 2 != 0 or any(before[i] != "-C" for i in range(0, len(before), 2)):
        raise GateError(
            f"`{path}`: before `--`, every argument must be a `-C <arg>` pair"
        )
    cargo_args = tuple(before[1::2])
    lib = False
    tests: list[str] = []
    index = 0
    while index < len(cargo_args):
        argument = cargo_args[index]
        if argument == "--lib":
            lib = True
            index += 1
        elif argument == "--test":
            if index + 1 >= len(cargo_args):
                raise GateError(f"`{path}`: `--test` needs a target name")
            target = cargo_args[index + 1]
            if target.startswith("-"):
                raise GateError(f"`{path}`: invalid test target `{target}`")
            tests.append(target)
            index += 2
        else:
            raise GateError(
                f"`{path}`: unsupported build target `{argument}` "
                "(only `--lib` and `--test` are allowed)"
            )
    if not lib and not tests:
        raise GateError(f"`{path}`: comment must filter build targets with `--lib` or `--test`")

    if len(after) != 2 or after[0] != "-E":
        raise GateError(f"`{path}`: after `--`, expected exactly `-E <filter>`")
    filter_expr = after[1].strip()
    if not filter_expr:
        raise GateError(f"`{path}`: the `-E` filter is empty")
    if filter_expr in {"all()", "none()"}:
        raise GateError(f"`{path}`: `{filter_expr}` is not a filtered test map")
    if not TEST_RE.search(filter_expr) and "binary(" not in filter_expr:
        raise GateError(f"`{path}`: filter must name `test(...)` or `binary(...)`")

    binaries = {
        match.group(1).strip().strip("\"'") for match in BINARY_RE.finditer(filter_expr)
    }
    missing_targets = sorted(binaries - set(tests))
    if missing_targets:
        raise GateError(
            f"`{path}`: filter names binary target(s) {missing_targets} "
            "but the comment does not contain `-C --test <name>` for them"
        )

    return Mapping(path, package, cargo_args, filter_expr)


def validate_tests(
    mapping: Mapping,
    root: Path,
    cache: dict[tuple[str, tuple[str, ...], str], int],
) -> int:
    """Check that the mapping selects at least one test; return the count."""
    key = (mapping.package, mapping.cargo_args, mapping.filter_expr)
    if key in cache:
        return cache[key]
    argv = [
        "cargo",
        "nextest",
        "list",
        "-p",
        mapping.package,
        *mapping.cargo_args,
        "-E",
        mapping.filter_expr,
    ]
    proc = run(argv, root)
    if proc.returncode != 0:
        detail = proc.stderr.strip().splitlines()
        tail = detail[-1] if detail else "unknown error"
        raise GateError(
            f"`{mapping.path}`: `cargo nextest list` failed: {tail}"
        )
    count = len([line for line in proc.stdout.splitlines() if line.strip()])
    if count == 0:
        raise GateError(f"`{mapping.path}`: the filter matches no tests")
    cache[key] = count
    return count


def normalize_path(raw: str, root: Path) -> Path:
    """Return a repo-relative path, accepting absolute and `./` forms."""
    path = Path(raw)
    if path.is_absolute():
        try:
            path = path.resolve().relative_to(root.resolve())
        except ValueError as err:
            raise GateError(f"`{raw}` is outside the repository") from err
    return Path(os.path.normpath(path))


def load_allowlist(path: Path, root: Path) -> dict[tuple[Path, int, int, str], AllowEntry]:
    """Read the accepted-equivalent allowlist, keyed by source coordinate."""
    if not path.is_file():
        raise GateError(f"allowlist `{path}` does not exist")
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as err:
        raise GateError(f"allowlist `{path}` is invalid TOML: {err}") from err
    raw_entries = data.get("allow", [])
    if not isinstance(raw_entries, list):
        raise GateError(f"allowlist `{path}`: `allow` must be an array of tables")
    allow: dict[tuple[Path, int, int, str], AllowEntry] = {}
    for index, raw in enumerate(raw_entries, 1):
        if not isinstance(raw, dict):
            raise GateError(f"allowlist `{path}`: entry {index} must be a table")
        file = raw.get("file")
        line = raw.get("line")
        column = raw.get("column")
        mutant = raw.get("mutant")
        reason = raw.get("reason")
        if not isinstance(file, str) or not file:
            raise GateError(f"allowlist `{path}`: entry {index} needs a `file` string")
        if type(line) is not int or line < 1:
            raise GateError(f"allowlist `{path}`: entry {index} needs a `line` integer")
        if type(column) is not int or column < 1:
            raise GateError(f"allowlist `{path}`: entry {index} needs a `column` integer")
        if not isinstance(mutant, str) or not mutant:
            raise GateError(f"allowlist `{path}`: entry {index} needs a `mutant` string")
        if not isinstance(reason, str) or not reason:
            raise GateError(f"allowlist `{path}`: entry {index} needs a non-empty `reason`")
        entry = AllowEntry(normalize_path(file, root), line, column, mutant, reason)
        if not (root / entry.path).is_file():
            raise GateError(f"allowlist `{path}`: `{entry.path}` does not exist")
        key = (entry.path, entry.line, entry.column, entry.mutant)
        if key in allow:
            raise GateError(
                f"allowlist `{path}`: duplicate entry for `{entry.path}:{entry.line}:{entry.column}`"
            )
        allow[key] = entry
    return allow


def select_files(args: argparse.Namespace, root: Path) -> tuple[list[Path], set[Path]]:
    """Resolve the requested files and the set of files cargo-mutants mutates."""
    mutated = mutated_files(root)
    if args.files:
        selected = [normalize_path(raw, root) for raw in args.files]
        for path in selected:
            if not (root / path).is_file():
                raise GateError(f"`{path}` does not exist")
    elif args.changed is not None:
        selected = sorted(changed_files(root, args.changed) & mutated)
    elif args.all:
        selected = sorted(mutated)
    else:
        raise GateError("specify files, `--changed [BASE]`, or `--all`")
    return selected, mutated


def collect_mappings(
    args: argparse.Namespace, root: Path, selected: list[Path], mutated: set[Path]
) -> tuple[list[Mapping], int]:
    """Parse and validate the comments for the selected files."""
    packages = cargo_metadata(root)
    cache: dict[tuple[str, tuple[str, ...], str], int] = {}
    mappings: list[Mapping] = []
    errors = 0
    for path in selected:
        if path not in mutated:
            print(f"skip {path} (no mutants)")
            continue
        try:
            package = package_for(path, root, packages)
            mapping = parse_comment(path, root, package)
            count = (
                0
                if args.no_test_validation
                else validate_tests(mapping, root, cache)
            )
        except GateError as err:
            if args.allow_missing and isinstance(err, MissingComment):
                print(f"warning: {err} (allowed by --allow-missing)")
                continue
            print(f"error: {err}", file=sys.stderr)
            errors += 1
            continue
        mappings.append(mapping)
        suffix = "" if args.no_test_validation else f" ({count} tests)"
        print(f"ok   {path} [{package}] {' '.join(mapping.cargo_args)} "
              f"-E {mapping.filter_expr}{suffix}")
    return mappings, errors


def suite_baseline(root: Path) -> None:
    """Run the full test suite once, so `--baseline skip` is safe."""
    argv = [
        "cargo",
        "nextest",
        "run",
        "--status-level",
        "fail",
        "--show-progress",
        "none",
    ]
    print(f"$ {shlex.join(argv)}")
    proc = run(argv, root, capture=False)
    if proc.returncode != 0:
        raise GateError(f"full test suite failed with status {proc.returncode}")


def union_filter(mappings: list[Mapping]) -> str:
    """Combine per-file filters with OR, parenthesising each one."""
    filters = [mapping.filter_expr for mapping in mappings]
    if len(filters) == 1:
        return filters[0]
    return " | ".join(f"({filter_expr})" for filter_expr in filters)


def missed_key(line: str, root: Path) -> tuple[Path, int, int, str] | None:
    """Split a `missed.txt` row into its allowlist key."""
    match = MISSED_RE.match(line)
    if match is None:
        return None
    return (
        normalize_path(match.group("file"), root),
        int(match.group("line")),
        int(match.group("column")),
        match.group("mutant"),
    )


def run_gate(
    args: argparse.Namespace,
    root: Path,
    mappings: list[Mapping],
    allowlist: dict[tuple[Path, int, int, str], AllowEntry],
) -> int:
    """Run one cargo-mutants invocation per build-target group."""
    groups: dict[tuple[str, ...], list[Mapping]] = {}
    for mapping in mappings:
        groups.setdefault(mapping.cargo_args, []).append(mapping)

    if not args.dry_run and not args.no_suite_baseline:
        try:
            suite_baseline(root)
        except GateError as err:
            print(f"error: {err}", file=sys.stderr)
            return 2

    failures = 0
    for index, (cargo_args, group) in enumerate(sorted(groups.items())):
        files = sorted(mapping.path for mapping in group)
        filter_expr = union_filter(group)
        output = args.output / f"group-{index:02d}"
        argv = ["cargo", "mutants"]
        for file in files:
            argv.extend(["-f", str(file)])
        argv.extend(
            [
                "--test-tool",
                "nextest",
                "--baseline",
                "skip",
                "--jobserver",
                "false",
                "-j",
                str(args.jobs),
                "-o",
                str(output),
            ]
        )
        if args.profile:
            argv.extend(["--profile", args.profile])
        for cargo_arg in cargo_args:
            argv.extend(["-C", cargo_arg])
        argv.extend(["--", "-E", filter_expr])

        env = os.environ.copy()
        if args.incremental:
            env["CARGO_INCREMENTAL"] = "1"

        print(f"$ {shlex.join(argv)}")
        if args.dry_run:
            continue
        shutil.rmtree(output, ignore_errors=True)
        output.mkdir(parents=True, exist_ok=True)
        proc = subprocess.run(argv, cwd=root, env=env)
        missed_file = output / "mutants.out" / "missed.txt"
        missed = []
        if missed_file.is_file():
            missed = [
                line.strip()
                for line in missed_file.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
        unallowed = []
        allowed = []
        for line in missed:
            key = missed_key(line, root)
            entry = allowlist.get(key) if key is not None else None
            if entry is None:
                unallowed.append(line)
            else:
                allowed.append((line, entry))
        if allowed:
            print(f"note: {len(allowed)} allowlisted equivalent mutant(s) in group {index:02d}:")
            for line, entry in allowed:
                print(f"  {line} ({entry.reason})")
        if unallowed:
            print(
                f"error: {len(unallowed)} missed mutant(s) in group {index:02d}:",
                file=sys.stderr,
            )
            for line in unallowed:
                print(f"  {line}", file=sys.stderr)
            failures += len(unallowed)
        elif proc.returncode != 0 and not missed:
            print(
                f"error: cargo-mutants group {index:02d} exited with status "
                f"{proc.returncode} (see {output / 'mutants.out'})",
                file=sys.stderr,
            )
            failures += 1

    if args.dry_run:
        print(f"dry run: {len(groups)} group(s)")
        return 0
    if failures:
        print(f"error: {failures} mutation failure(s)", file=sys.stderr)
        return 1
    print("mutation gate clean")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate and run cargo-mutants from per-file `// mutants:` comments."
    )
    parser.add_argument("files", nargs="*", help="source files to use")
    parser.add_argument(
        "--root",
        default=None,
        help="repository root (default: the tool's parent's parent)",
    )
    parser.add_argument(
        "--changed",
        nargs="?",
        const="master",
        default=None,
        metavar="BASE",
        help="use source files changed since BASE (default: master)",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="use every source file that cargo-mutants would mutate",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate comments only (the default)",
    )
    parser.add_argument(
        "--run",
        action="store_true",
        help="run cargo-mutants after validating the comments",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=4,
        help="cargo-mutants jobs (default: 4)",
    )
    parser.add_argument(
        "--profile",
        default=None,
        help="cargo profile passed to cargo-mutants",
    )
    parser.add_argument(
        "--incremental",
        action="store_true",
        help="set CARGO_INCREMENTAL=1 for the mutation runs",
    )
    parser.add_argument(
        "--no-suite-baseline",
        action="store_true",
        help="skip the full nextest baseline before the mutation runs",
    )
    parser.add_argument(
        "--no-test-validation",
        action="store_true",
        help="skip `cargo nextest list` (syntax check only)",
    )
    parser.add_argument(
        "--allowlist",
        default=DEFAULT_ALLOWLIST,
        metavar="PATH",
        help=f"accepted-equivalent mutants (default: {DEFAULT_ALLOWLIST})",
    )
    parser.add_argument(
        "--no-allowlist",
        action="store_true",
        help="ignore the accepted-equivalent allowlist",
    )
    parser.add_argument(
        "--allow-missing",
        action="store_true",
        help="warn, instead of failing, on files without a comment",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print the cargo-mutants commands without running them",
    )
    parser.add_argument(
        "--output",
        default="mutants.out.gate",
        help="directory for per-group cargo-mutants output (default: mutants.out.gate)",
    )
    args = parser.parse_args(argv)

    if args.check and args.run:
        parser.error("--check and --run are mutually exclusive")
    if args.jobs < 1:
        parser.error("--jobs must be at least 1")
    if sum(bool(value) for value in (args.files, args.changed is not None, args.all)) > 1:
        parser.error("choose one of: files, --changed, --all")

    root = (
        Path(args.root)
        if args.root is not None
        else Path(__file__).resolve().parent.parent
    )
    root = root.resolve()
    if not root.is_dir():
        print(f"error: repository root '{root}' is not a directory", file=sys.stderr)
        return 2
    args.output = (root / args.output).resolve()

    try:
        allowlist = (
            {}
            if args.no_allowlist
            else load_allowlist((root / args.allowlist).resolve(), root)
        )
        selected, mutated = select_files(args, root)
        mappings, errors = collect_mappings(args, root, selected, mutated)
    except GateError as err:
        print(f"error: {err}", file=sys.stderr)
        return 2

    if errors:
        print(f"error: {errors} comment validation failure(s)", file=sys.stderr)
        return 1
    if not args.run:
        print(f"checked {len(mappings)} mapping(s)")
        return 0
    if not mappings:
        print("nothing to run")
        return 0
    return run_gate(args, root, mappings, allowlist)


if __name__ == "__main__":
    raise SystemExit(main())
