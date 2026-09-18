---
name: mutants
description: Kill all mutants in this repo with cargo mutants. Runs the full mutation-testing loop — rank files by missed mutants, add tests or refactor equivalent-mutant sources, QA-review and commit per file, repeat until only unkillable mutants remain. Use when asked to run mutation testing, catch mutants, or improve mutant coverage.
---

# Mutation testing loop

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

3. **Verify** the file is clean:
   `cargo mutants -j4 --test-tool nextest -f <file> --iterate`
   and confirm the file no longer appears in `mutants.out/missed.txt`.

4. **Finish the file.** When the file is fully caught:
   - `git add -N` new files (so nix builds see them) and `git add` the changed
     files (source + tests + LESSONS.md)
   - Run the **reviewer** subagent's QA checklist (`.opencode/agent/reviewer.md`
     §5) over the changed files and fix what it flags
   - Do **not** commit — the Judge commits this work when the task reaches
     `Done` (AGENTS.md).

5. **Re-run** the mutation check for the next iteration:
   `cargo mutants -j4 --test-tool nextest --iterate`
   (or a full run if `--iterate` state is stale), then back to step 1.

## Stopping

Stop the loop when every remaining entry in `mutants.out/missed.txt` is an
equivalent mutant — document each in `LESSONS.md` with why it is unkillable,
`git add` those docs and the changed sources (the Judge commits via the
workflow when the task reaches `Done`), and report the final residual list.
