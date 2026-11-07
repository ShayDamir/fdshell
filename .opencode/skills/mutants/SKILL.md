---
name: mutants
description: Kill mutants with the comment-driven gate or a full cargo-mutants sweep. Runs the mutation-testing loop — rank files by missed mutants, add tests or refactor equivalent-mutant sources, record accepted equivalents in tools/mutants_allowlist.toml, QA-review and commit per file. Use when asked to run mutation testing, catch mutants, or improve mutant coverage.
---

# Mutation testing loop

Before accepting any mutant as equivalent, consult
[`CATCHING_MUTANTS.md`](../../../CATCHING_MUTANTS.md) — the growing list of
recipes for catching missed mutants. Extend it whenever a new probe shape
works or a recorded equivalence is disproved.

There are two scopes:

- **Changed-file gate (every task).** This is the only run an ordinary task
  needs: `python3 tools/mutants_gate.py --run --changed master` uses each
  changed file's `// mutants:` map. It must be clean before Review.
- **Full sweep (Chore only).** A full `cargo mutants` run is too slow to gate
  every task, so it lives only inside a Chore (AGENTS.md "Chore workflow"). The
  Chore flows through the normal pipeline; when the Judge moves it to `Done`
  it re-spawns a fresh copy in `Backlog`, so the sweep recurs without anyone
  re-creating it.

## Maps and allowlist

A changed mutated source file must carry exactly one `// mutants:` line (the
full set accrues task by task; do not bulk-annotate):

```
// mutants: -C --lib -C --test -C timeout -- -E 'test(/^intercept::timeout_cmd::tests::/) | binary(timeout)'
```

The payload is exactly the cargo-mutants arguments: every `-C` pair names a
cargo/nextest build target, and the arguments after `--` are passed to nextest.
Keep the filter anchored at the module or crate prefix, never at a `tests`
submodule: use `test(/^parse::case_block::/)`, not
`test(/^parse::case_block::tests::/)`, or nested test modules silently stop
running.

An equivalent mutant goes in `tools/mutants_allowlist.toml` with the exact
file, line, column, cargo-mutants description and a reason/proof. `LESSONS.md`
keeps the reusable lesson and pins; the allowlist is the active registry.
Remove an entry as soon as the site changes — a stale proof is a new finding,
not a licence.

## Commands

```
# changed-file gate (ordinary task)
python3 tools/mutants_gate.py --check --changed master  # validate maps only
python3 tools/mutants_gate.py --run --changed master    # run the gate
# add --no-suite-baseline after a clean full nextest run

# full sweep (Chore)
python3 tools/mutants_gate.py --run --all  # once every mutated file has a map
cargo mutants -j4 --test-tool nextest          # full run (up to 20 min)
cargo mutants -j4 --test-tool nextest -F <dir>  # focus on a directory
cargo mutants -j4 --test-tool nextest -f <file> # focus on a single file
cargo mutants -j4 --test-tool nextest -f <file1> -f <file2> ... -f <fileN>
```

The gate writes its misses under
`mutants.out.gate/group-*/mutants.out/missed.txt`; a raw `cargo mutants` run
uses `mutants.out/missed.txt`. `--iterate` re-runs only previously missed
mutants — use it to speed up raw check runs after fixing some mutants (e.g.
`-f <file> --iterate`).

## Changed-file gate

Run `python3 tools/mutants_gate.py --run --changed master`. A missing
`// mutants:` map is a failure; add it when the task first touches the file,
and do not bulk-annotate unrelated files. For each miss:

1. Read the row and the source.
2. Kill it with a test (tests live in separate `<module>/tests.rs` files per
   AGENTS.md), or refactor the source if the mutant is equivalent. Apply the
   recipes in `CATCHING_MUTANTS.md` first.
3. Only after a proof of equivalence, add the exact
   `tools/mutants_allowlist.toml` entry.
4. Re-run the gate for the changed files.

## Full sweep loop (Chore)

Repeat until `missed.txt` contains only allowlisted equivalents:

1. **Pick the worst file.**
   ```
   cut -d: -f1 mutants.out/missed.txt | sort | uniq -c | sort -rn
   ```
   Work on the file with the most missed mutants.

2. **Read the mutants** for that file (`grep '^<file>:' mutants.out/missed.txt`)
   and the source. For each mutant, either:
   - **Add a test** that kills it, or
   - **Refactor the source** if the mutant is *equivalent* to the original
     code (unkillable). Remove the source of the equivalence rather than
     accepting it; if the equivalence remains, add the exact allowlist entry
     and record the generalized rule in `LESSONS.md` (see existing entries on
     equivalent mutants there).
   Apply the recipes in `CATCHING_MUTANTS.md` first; add a new rule there when
   a probe shape proves useful.

3. **Verify** the file is clean:
   `cargo mutants -j4 --test-tool nextest -f <file> --iterate`
   and confirm the file no longer appears in `mutants.out/missed.txt`.

4. **Finish the file.** When the file is fully caught:
    - `git add` new and changed files (source + tests + `LESSONS.md` +
      `CATCHING_MUTANTS.md` + `tools/mutants_allowlist.toml`) on the task
      branch `taskN` (so nix builds see them)
    - Run the **reviewer** subagent's QA checklist (`.opencode/agent/reviewer.md`
      §5) over the changed files and fix what it flags
    - Commit the work on `taskN` (AGENTS.md: per-task branches) — the Judge
      merges the branch when the task reaches `Done`.

5. **Re-run** the mutation check for the next iteration:
   `cargo mutants -j4 --test-tool nextest --iterate`
   (or a full run if `--iterate` state is stale), then back to step 1.

## Stopping

Stop the loop when every remaining entry in the missed list has an exact
`tools/mutants_allowlist.toml` entry with its proof. Put any new catching
recipe in `CATCHING_MUTANTS.md`, and a reusable lesson in `LESSONS.md` when the
equivalence teaches one. `git add` and commit those docs, the allowlist and
the changed sources on the task branch `taskN` (the Judge merges the branch via
the workflow when the task reaches `Done`), and report the final residual list.
