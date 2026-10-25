---
name: mutants
description: Kill all mutants in this repo with cargo mutants. Runs the full mutation-testing loop — rank files by missed mutants, add tests or refactor equivalent-mutant sources, QA-review and commit per file, repeat until only unkillable mutants remain. Use when asked to run mutation testing, catch mutants, or improve mutant coverage.
---

# Mutation testing loop

Before accepting any mutant as equivalent, consult
[`CATCHING_MUTANTS.md`](../../../CATCHING_MUTANTS.md) — the growing list of
recipes for catching missed mutants. Extend it whenever a new probe shape
works or a recorded equivalence is disproved.

Run this as a **Chore** task, not as a gate on an ordinary task: a full sweep
is too slow to gate every task, so mutation testing lives only inside a Chore
(AGENTS.md "Chore workflow"). The Chore flows through the normal pipeline; when
the Judge moves it to `Done` it re-spawns a fresh copy in `Backlog`, so the
sweep recurs without anyone re-creating it.

## Commands

```
cargo mutants -j4 --test-tool nextest          # full run (up to 20 min)
cargo mutants -j4 --test-tool nextest -F <dir>  # focus on a directory
cargo mutants -j4 --test-tool nextest -f <file> # focus on a single file
cargo mutants -j4 --test-tool nextest -f <file1> -f <file2> ... -f <fileN>  # run on multiple files
```

`--iterate` mode re-runs only previously missed mutants — use it to speed up
check runs after fixing some mutants (e.g. after adding tests, verify with
`-f <file> --iterate` before moving on).

After a successful run, missed mutants are listed in `mutants.out/missed.txt`,
one per line: `<path>:<line>:<col>: <description>`.

## Loop

Repeat until `missed.txt` contains only unkillable (equivalent) mutants:

1. **Pick the worst file.**
   ```
   cut -d: -f1 mutants.out/missed.txt | sort | uniq -c | sort -rn
   ```
   Work on the file with the most missed mutants.

2. **Read the mutants** for that file (`grep '^<file>:' mutants.out/missed.txt`)
   and the source. For each mutant, either:
   - **Add a test** that kills it (tests live in separate `<module>/tests.rs`
     files per AGENTS.md), or
   - **Refactor the source** if the mutant is *equivalent* to the original
     code (unkillable). Remove the source of the equivalence rather than
     accepting it. Record a new lesson in `LESSONS.md` (see existing entries
     on equivalent mutants there).
   Apply the recipes in `CATCHING_MUTANTS.md` first; add a new rule there when
   a probe shape proves useful.

3. **Verify** the file is clean:
   `cargo mutants -j4 --test-tool nextest -f <file> --iterate`
   and confirm the file no longer appears in `mutants.out/missed.txt`.

4. **Finish the file.** When the file is fully caught:
    - `git add` new and changed files (source + tests + `LESSONS.md` +
      `CATCHING_MUTANTS.md`) on the task branch `taskN` (so nix builds see them)
    - Run the **reviewer** subagent's QA checklist (`.opencode/agent/reviewer.md`
      §5) over the changed files and fix what it flags
    - Commit the work on `taskN` (AGENTS.md: per-task branches) — the Judge
      merges the branch when the task reaches `Done`.

5. **Re-run** the mutation check for the next iteration:
   `cargo mutants -j4 --test-tool nextest --iterate`
   (or a full run if `--iterate` state is stale), then back to step 1.

## Stopping

Stop the loop when every remaining entry in `mutants.out/missed.txt` is an
equivalent mutant — document each in `LESSONS.md` with why it is unkillable,
and put any new catching recipe in `CATCHING_MUTANTS.md`, `git add` and commit
those docs and the changed sources on the task branch `taskN` (the Judge merges
the branch via the workflow when the task reaches `Done`), and report the final
residual list.
