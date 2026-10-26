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
   nonzero neighbour. When the equality case *is* the no-op, no test can kill
    it: rewrite the comparison as a predicate (`is_negative()`,
    `cmp(..) == Ordering::Less`) or an explicit delta tested `!= 0`, so the
    surviving mutants must force a value, then pin both sides of that
    predicate. When a guard **conjunct** is implied by an invariant of the
    surrounding state, delete the conjunct instead: the surviving comparison
    carries the whole rule, so its mutants become killable (the glob
    backtrack guard's `sn < name.len()` was implied by `ni >= sn`; after the
    deletion its `<`→`<=` twin fails the star-backtracking pins).

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
   for compound guards so `&&`→`||` becomes killable. The same trap sits on
   disjoint *value ranges* — a masked fraction under a shifted nibble — so sum
   those with `+` too, where `+`→`-`/`*` moves the value and an exact-output
   pin kills it.
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
    re-derived, remove the duplicate and pin the fall-through. The duplicate
    may live in a *later* stage: a lexer guard that bails on what the parser's
    trailing-input rule also rejects is a duplicate, and deleting it moves
    only the `╰╴at …` source-location line — rule 5 says that is not a
    behavioral difference, so pin the surviving message and exit status.
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
15. **Delete a dead gate at the call site.** A guard whose every mutant is
    equivalent encodes no rule. Deleting just the callee leaves the caller's
    `&&` and its call site; removing the caller's conjunct deletes the
    callee's whole mutant family at once. Pin the scope the gate claimed to
    check, balanced *and* unbalanced, so the answer stays where it was ever
    observable.
16. **Name the predicate.** A comparison against a value the scorer can never
    return carries a family no test can kill: `d > 0` over a sign projection
    with no `0` row mutates to `>=`/`==`/`<` and every mutant accepts the same
    set. Write `d.is_positive()`: an argument-less predicate method exposes no
    operator, so the family is absent from `--list` (measured on
    `parse/time_block.rs::inner_text`, the `keyword_delta` sign projection).
    Keep the sentence naming the invariant that makes the comparison a
    no-op-equivalent, plus the pin that notices if the invariant dies.
17. **Share the duplicated value with the load-bearing site.** A mutant
    survives because a sibling recomputes the same expression and converges:
    the star's resume `(pi + 1, ni)` was recorded *and* stepped forward, so
    mutating the record only re-read the star, and the forward step rebuilt the
    state. Compute the value once into a local and use it at both sites, so the
    mutation hits the live consumer too: forward-progress loss becomes a
    deterministic livelock (killed by nextest's `slow-timeout`; attribute it to
    the pins that time out — 14 `glob::` rows here), an index shift becomes an
    exact-index kill (`pi - 1` underflows, and the star pins fail).
