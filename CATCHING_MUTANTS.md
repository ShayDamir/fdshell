# Catching mutants

Reusable recipes for turning missed mutants into caught ones. Keep this file
short. Add a rule whenever a probe kills a mutant or disproves an
"accepted equivalent" entry.

1. **Apply the mutant as source.** For an identity-shaped mutant (`x + 1` →
   `x * 1`, `x - 0`, `x * 1`, `x | 0`, `x && true`), replace the original with
   the identity form, run the suite, and compare behavior on real input. If
   anything changes, the mutant was never equivalent: keep the original and
   add the probe as a test. Example: `close + 1` → `close` in the bracket
   class skip changed `${v#[[:alpha:]\]]}` from `[]` to `[]]` while all 5863
   tests passed.
2. **Probe the mutated state, not only the outcome.** Unit-test the changed
   index, flag, or counter directly; end-to-end paths often normalize the
   difference away.
3. **Hit both sides of every comparison.** For `<`/`<=`, `>`/`>=`, `==`/`!=`,
   and `+`/`-`/`*`, test the equality/boundary case and the adjacent case. If
   equality makes the work a no-op (zero width, zero repeat), also pin a
   nonzero neighbour.
4. **Kill the value where it is observable.** Follow the mutated value to its
   consumer (error text, byte count, output byte) instead of testing the
   computation site.
5. **Use a differential oracle.** Run the smallest input against clean and
   mutated builds (or bash) and compare stdout, stderr, and exit status; keep
   that input as the regression test. A differing source-location line alone
   is not a behavioral difference; compare the message and status.
6. **Remove dead equivalence; do not test around it.** If a caller invariant
   makes an arm unreachable, delete the arm or restructure the caller and
   record the invariant in `LESSONS.md`.
7. **Re-derive equivalences after refactors.** Moving a loop or splitting a
   file invalidates old proofs; re-run their simplification/probe checks
   against the new shape.
8. **Truth-table booleans and bit operations.** For `&&`/`||`/`!`, test each
   operand combination. For `|`/`^`/`&`, check bit overlap: use `+` for
   disjoint non-zero flags, drop zero flags (`O_RDONLY`), and compare tuples
   for compound guards so `&&`→`||` becomes killable.
9. **Pin exact state, lists, and cursors.** End-to-end output hides
   index/cursor differences; assert exact token lists, window ends, and cursor
   values. The class-skip bug in rule 1 passed the whole suite because no pin
   covered the cursor state.
10. **Prove reachability first.** Find the input that reaches the arm; a guard
    whose true arm returns the fall-through value is dead — delete it. An
    "equivalent because the input is rejected" claim is temporary: name the
    rejecting rule and re-check when it changes.
11. **Delete duplicate rules.** If two rules compute the same advancement or
    clamp, keep one. If a callee already enforces a guard, or a flag is
    re-derived, remove the duplicate and pin the fall-through.
12. **Clamps hide signs.** A `max(0)` on the closer does not make the opener's
    `+=`→`-=` equivalent; exercise nested opens, then a closer, then a
    boundary.
13. **Derive constants from data; pin each term.** Prefer `b"time".len()` to a
    literal, and pin each whitespace/offset term with an exact length. Integer
    literals are mutant sources.
14. **Do not trust a parallel sweep.** `cargo mutants` runs only the mutated
    package's tests by default — use `--test-workspace true` or a per-crate
    test. Re-derive every accepted/killed claim by hand or at `-j1`; a flaky
    timeout is a false kill. After a split, include every file in the focused
    run.
