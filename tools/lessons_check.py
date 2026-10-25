#!/usr/bin/env python3
"""LESSONS.md health gate: cross-references, code references, section sizes.

LESSONS.md is binding reading for every agent (AGENTS.md line 3), and its
accepted-equivalent-mutant entries are the registry the `mutants` skill reads,
yet nothing checked its health. This tool reports, and with `--check` fails
(exit code 1) on:

  * a `LESSONS: "<heading prefix>"` cross-reference that matches no heading as
    a case-insensitive, whitespace-normalised prefix;
  * a `*.rs` path reference that resolves to no file in the tree (suffix match,
    so `scan/heredoc/ops.rs` resolves under `safe/fdshell/src/...`).

Reported, never failing:

  * `LESSONS:` citations not in the canonical quoted form (informal prose);
  * `path.rs::ident` references whose last segment is absent from the file;
  * `## ` sections above SECTION_BUDGET_BYTES, so growth stays visible.

Exit codes: 0 = clean, 1 = violations with `--check`, 2 = usage/environment
error (root not a directory, no LESSONS.md, no `*.rs` in the tree) — the same
contract as `tools/complexity.py`.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path
from typing import NamedTuple

SECTION_BUDGET_BYTES = 4000  # the trimming chore's per-entry target (#442)
CONVENTIONS_PREFIX = "conventions for writing entries"  # preamble: cites the format

CITE_WINDOW = 120  # chars scanned after `LESSONS:` for the quoted heading
CITE_SEPARATORS = " \t\n()[]*`"  # allowed between `LESSONS:` and its quote
MAX_CITE_GAP = 8  # separators tolerated before the opening quote (a wrapped citation)
TEMPLATE_CHARS = "<>{}*?[]~$|"  # `<module>/tests.rs`, `{a,b}.rs` are templates
TEMPLATE_LOOKBACK = 60  # chars scanned back to the nearest span boundary
INFORMAL_EXCERPT = 60  # report width for an informal citation

HEADING_RE = re.compile(r"^#{1,6} +(.+?)[ \t]*$")
SECTION_RE = re.compile(r"^## +(.+?)[ \t]*$")
CITE_RE = re.compile(r"LESSONS:")
RS_RE = re.compile(r"[A-Za-z0-9_][A-Za-z0-9_:./+-]*\.rs")
IDENT_RE = re.compile(r"([A-Za-z0-9_][A-Za-z0-9_:./+-]*\.rs)((?:::[A-Za-z0-9_]+)+)")


class Citation(NamedTuple):
    line: int
    text: str
    ok: bool


def norm(text: str) -> str:
    """Case-fold and collapse whitespace, so a citation matches its heading."""
    return re.sub(r"\s+", " ", text).strip().casefold()


def line_of(text: str, pos: int) -> int:
    return text.count("\n", 0, pos) + 1


def heading_titles(text: str) -> list[str]:
    return [m.group(1) for m in (HEADING_RE.match(ln) for ln in text.splitlines()) if m]


def conventions_span(lines: list[str]) -> tuple[int, int]:
    """Return the [start, end) line window (1-based) of the preamble section.

    The preamble documents the citation format itself ("`LESSONS:` + a quoted
    heading prefix"), so scanning it as a citation is a false positive.
    """
    for idx, ln in enumerate(lines, 1):
        match = SECTION_RE.match(ln)
        if match and norm(match.group(1)).startswith(CONVENTIONS_PREFIX):
            for end in range(idx + 1, len(lines) + 1):
                if SECTION_RE.match(lines[end - 1]):
                    return (idx, end)
            return (idx, len(lines) + 1)
    return (0, 0)


def quoted_ref(text: str, start: int) -> tuple[int, str]:
    """Find the quote opening at/after `start`; (-1, excerpt) when informal.

    A citation may wrap (the live file contains one), so up to MAX_CITE_GAP
    separator characters, newlines included, are skipped before the quote.
    """
    tail = text[start : start + CITE_WINDOW]
    gap = len(tail) - len(tail.lstrip(CITE_SEPARATORS))
    if gap >= len(tail) or gap > MAX_CITE_GAP or tail[gap] != '"':
        return -1, re.sub(r"\s+", " ", tail.lstrip()).replace('"', "'")
    close = tail.find('"', gap + 1)
    if close < 0:  # quote never closes inside the window
        return -1, tail[gap + 1 :][:INFORMAL_EXCERPT]
    return gap + 1, tail[gap + 1 : close]


def citations(text: str, titles: list[str], skip: tuple[int, int]) -> tuple[list[Citation], list[Citation]]:
    """Split citations into (quoted, informal); `ok` says a heading matches."""
    prefixes = [norm(t) for t in titles]
    quoted: list[Citation] = []
    informal: list[Citation] = []
    for match in CITE_RE.finditer(text):
        line = line_of(text, match.start())
        if skip[0] <= line < skip[1]:
            continue
        offset, ref = quoted_ref(text, match.end())
        ref = re.sub(r"\s+", " ", ref).strip()
        if offset < 0:
            informal.append(Citation(line, ref[:INFORMAL_EXCERPT], True))
        else:
            quoted.append(Citation(line, ref, any(p.startswith(norm(ref)) for p in prefixes)))
    return quoted, informal


def rs_index(root: Path) -> dict[str, list[str]]:
    """Index every `*.rs` under root by basename -> repo-relative paths.

    Only the top-level `target/` build directory is pruned: the source tree
    legitimately contains a nested `target/` package (STYLE.md 2.9), and a
    blanket skip of every directory named `target` mis-reports its files as
    missing. Symlinked directories are not followed (`os.walk` default).
    """
    index: dict[str, list[str]] = {}
    base = root.resolve()
    for dirpath, dirnames, filenames in os.walk(base):
        current = Path(dirpath)
        dirnames[:] = sorted(
            name for name in dirnames
            if name != ".git" and not (current == base and name == "target")
        )
        for name in sorted(filenames):
            if name.endswith(".rs"):
                rel = (current / name).relative_to(base).as_posix()
                index.setdefault(name, []).append(rel)
    return index


def resolve(path: str, index: dict[str, list[str]]) -> list[str]:
    """Repo-relative candidates for a reference, matching on the path suffix."""
    normalized = path.replace("::", "/")
    name = Path(normalized).name
    return [r for r in index.get(name, []) if r == normalized or r.endswith("/" + normalized)]


def is_template(text: str, start: int) -> bool:
    """True when the reference sits inside a template: `<module>/tests.rs`.

    The path regex cannot see `<`, `{` or `*`, so a template leaks its tail
    (`<x>/nope.rs` matches `nope.rs`). A template fragment is a reference that
    shares its code span with a placeholder marker, so scanning the span back
    to the nearest whitespace/backtick boundary for one is enough.
    """
    span = text[max(0, start - TEMPLATE_LOOKBACK) : start]
    cut = max(span.rfind(ch) for ch in " \t\n`")
    return any(ch in span[cut + 1 :] for ch in TEMPLATE_CHARS)


def path_references(text: str, index: dict[str, list[str]]) -> list[tuple[int, str]]:
    """`*.rs` references that resolve to no file in the tree (first mention)."""
    out: list[tuple[int, str]] = []
    seen: set[str] = set()
    for match in RS_RE.finditer(text):
        ref = match.group(0)
        if is_template(text, match.start()) or ref in seen:
            continue
        seen.add(ref)
        if not resolve(ref, index):
            out.append((line_of(text, match.start()), ref))
    return out


def ident_references(text: str, root: Path, index: dict[str, list[str]]) -> list[tuple[int, str]]:
    """`path.rs::ident` references whose last segment is absent from the file."""
    cache: dict[str, str] = {}
    out: list[tuple[int, str]] = []
    seen: set[str] = set()
    for match in IDENT_RE.finditer(text):
        path, chain = match.group(1), match.group(2)
        ident = chain.split("::")[-1]
        key = f"{path}::{ident}"
        if is_template(text, match.start()) or key in seen:
            continue
        candidates = resolve(path, index)
        if not candidates:
            continue  # the path itself is reported by path_references()
        seen.add(key)
        blob = "".join(
            cache.setdefault(cand, (root / cand).read_text(encoding="utf-8", errors="replace"))
            for cand in candidates
        )
        # a trailing `_` in LESSONS prose is a test-name prefix, not a full name
        tail_word = "" if ident.endswith("_") else r"\b"
        if not re.search(rf"\b{re.escape(ident)}{tail_word}", blob):
            out.append((line_of(text, match.start()), key))
    return out


def sections(text: str) -> list[tuple[int, str]]:
    """`## ` sections as (bytes, title), including the heading line.

    The heading line is counted on purpose: the gate's number is the cost of
    reading the section, and it differs from the chore's body-only count by
    exactly one heading line.
    """
    lines = text.splitlines(keepends=True)
    marks = [(i, SECTION_RE.match(ln).group(1)) for i, ln in enumerate(lines) if SECTION_RE.match(ln)]
    sized = []
    for pos, (start, title) in enumerate(marks):
        end = marks[pos + 1][0] if pos + 1 < len(marks) else len(lines)
        sized.append((len("".join(lines[start:end]).encode()), title))
    return sized


def print_report(lessons: Path, titles: list[str], quoted: list[Citation], informal: list[Citation],
                 dangling: list[tuple[int, str]], unresolved_paths: list[tuple[int, str]],
                 missing_idents: list[tuple[int, str]], all_sections: list[tuple[int, str]], budget: int) -> None:
    """Print the developer-facing report; `--check` adds one stderr summary."""
    print(f"LESSONS.md health on {lessons}")
    print(f"{len(titles)} heading(s), {lessons.stat().st_size} byte(s).")
    print()
    print("Dangling cross-references (must match a heading as a prefix):")
    for line, ref in dangling:
        print(f"  line {line}: LESSONS: \"{ref}\"")
    print(f"({len(dangling)} of {len(quoted)} quoted citation(s))")
    print()
    print("Unresolved `*.rs` references (no file in the tree, suffix match):")
    for line, ref in unresolved_paths:
        print(f"  line {line}: {ref}")
    print(f"({len(unresolved_paths)} reference(s))")
    print()
    print("REPORT ONLY ---------------------------------------------------------")
    print("Informal citations (canonical form is LESSONS: \"<heading prefix>\"):")
    for cite in informal:
        print(f"  line {cite.line}: LESSONS: {cite.text}")
    print(f"({len(informal)} citation(s))")
    print()
    print("Ident references `path.rs::ident` absent from the resolved file:")
    for line, ref in missing_idents:
        print(f"  line {line}: {ref}")
    print(f"({len(missing_idents)} reference(s))")
    print()
    big = sorted(((size, title) for size, title in all_sections if size > budget),
                 key=lambda item: (-item[0], item[1]))
    print(f"Sections above {budget} B (trimming is the standing chore):")
    for size, title in big:
        print(f"  {size:>6} B  {title}")
    print(f"({len(big)} of {len(all_sections)} section(s))")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check LESSONS.md cross-references, code references and section sizes."
    )
    parser.add_argument(
        "root",
        nargs="?",
        default=None,
        help="repository root (default: the tool's parent's parent)",
    )
    parser.add_argument(
        "--lessons",
        default=None,
        help="LESSONS.md to check (default: <root>/LESSONS.md)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 on dangling cross-references or unresolved `*.rs` references",
    )
    parser.add_argument(
        "--budget",
        type=int,
        default=SECTION_BUDGET_BYTES,
        help="report-only per-section byte budget",
    )
    args = parser.parse_args(argv)

    root = Path(args.root) if args.root is not None else Path(__file__).resolve().parent.parent
    if not root.is_dir():
        print(f"error: repository root '{root}' is not a directory", file=sys.stderr)
        return 2
    lessons = Path(args.lessons) if args.lessons is not None else root / "LESSONS.md"
    if not lessons.is_file():
        print(f"error: {lessons} not found", file=sys.stderr)
        return 2

    text = lessons.read_text(encoding="utf-8")
    titles = heading_titles(text)
    quoted, informal = citations(text, titles, conventions_span(text.splitlines()))
    index = rs_index(root)
    if not index:
        print(f"error: no `*.rs` file under '{root}'; is it a repository tree?", file=sys.stderr)
        return 2

    dangling = [(c.line, c.text) for c in quoted if not c.ok]
    unresolved_paths = path_references(text, index)
    missing_idents = ident_references(text, root, index)
    print_report(lessons, titles, quoted, informal, dangling, unresolved_paths,
                 missing_idents, sections(text), args.budget)

    if args.check and (dangling or unresolved_paths):
        print(
            f"error: {len(dangling)} dangling cross-reference(s), "
            f"{len(unresolved_paths)} unresolved `*.rs` reference(s)",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
