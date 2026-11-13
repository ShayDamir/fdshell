# Catching mutants

Reusable recipes for turning missed mutants into caught ones. Keep this file
short. Add a rule whenever a probe kills a mutant or disproves an
"accepted equivalent" entry.

Active accepted equivalents are recorded in
[`tools/mutants_allowlist.toml`](tools/mutants_allowlist.toml); `LESSONS.md`
keeps the reusable proof. A "registry" below means that allowlist.

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
   pin kills it. Measured both ways on task #554: the assign arm of
   `parse/umask_mode/symbolic.rs` sums two operands the callee's own mask keeps
   disjoint, so it is written `+` and its mutants die on the exact-mask pins;
   the `|` inside an *accumulating* loop is the killable one — feed the loop
   twice with the same bit (`a+rwr`, pinned in
   `safe/fdshell/src/parse/umask_mode/symbolic/tests.rs`).
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
    may be a **rebinding** of a value the code above already established —
    `let esac_idx = tokens.len() - 1;` written after `tokens.get(..=esac_idx)`
    truncated the slice states the same number twice and hands `mutants` a
    fresh offset to move; delete the binding and use the original name. The
    duplicate may live in a *later* stage: a lexer guard that bails on what
    the parser's trailing-input rule also rejects is a duplicate, and deleting
    it moves
    only the `╰╴at …` source-location line — rule 5 says that is not a
    behavioral difference, so pin the surviving message and exit status. A
    test repeated in **every arm** of a `match` is the same duplicate: hoist
    it into the early return so each arm states only its own rule, and the
    per-arm `&&` site (with its `&&`→`||` family) disappears; the hoisted
    gate's mutants die on the arms' existing rows. Check the residue: an arm
    whose one remaining test can only *block* a byte class the matcher
    rejects anyway is observable in one direction — take it `with true`, watch
    the suite stay green, and record that mutant (rule 6).
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
    observable. The gate may be a **match-arm guard** that restates the
    callee's own loop test: fold the arm into the catch-all so the callee's
    single test carries the rule (its mutants then die on the name rows), and
    pin the fallback the fold inherits, including the byte at the **end of the
    text** (`a%`, `%-`, `%`) — the mid-word rows hide a byte that is consumed
    instead of pushed.
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
18. **Delete a bound the searches already enforce.** A loop bound whose range
    only re-probes tokens that cannot match the needle is a shield: it carries
    `<`→`<=` (and `+`/`-`) mutants no test can see, while the body's own
    searches already stop the walk. Write the loop as `loop` plus the
    searches' `break`, and keep the searches' ranges — a range carries no
    operators. Prefer that to a slice rewrite: `get(a..)` with an
    `unwrap_or_default`/`unwrap_or` fallback re-creates the equivalence,
    because `replace … with None` **is** the fallback. Measured on
     `parse/case_block/literal.rs`: the bound and rule 11's rebinding both kept
     the suite green, deleting them deleted both families, and the step base
     shared with the output range is killed by the exact-index pins (rule 9).
     When the body carries no test of its own (a pure state-machine walk), end
     the loop through the exact range's own method — `loop { if (i..next).is_empty()
     { break } … }` — measured on task #513 (`scan/heredoc/ops.rs`'s word walk, the
     three `scan/heredoc/lines.rs` walks).
19. **Program-level unreachability does not bind a helper-level call.** Rule 10's
    "equivalent because the input is rejected upstream" is a claim about the
    *program*; a unit test feeds the helper exactly the text the rejection
    forbids. Pin the helper's own answer there and the identity dies:
    `cond/boundary/body`'s window end `nl + 1` → `nl * 1` (recorded equivalent
    because an unterminated-quote delimiter word never reaches the cond layer)
    fails a direct `window` row that asserts the exact `(from, to)`, region list
    and resume — at `to == nl` the quote region stops at the window edge and the
    delimiter reads `a b` (a match), one past it the quote swallows the newline
    byte and nothing matches. Same for `scan/heredoc/line_start`'s `map(|n| n +
    1)`: the identity answers with the newline's own index, and
    `scan/heredoc/opaque`'s region filter compares that anchor with the window's
    start, so under the mutant the terminator line's own `<<` is counted and the
    opaque count collapses to the plain count.
20. **Make a size constant a type.** A size computed by arithmetic
    (`header + payload + pad`) is a `+`→`*` family that over-reserves
    *invisibly* — the kernel is told the *needed* length and writes only what the
    message carries — and its `+`→`-` twin under-reserves, so the kernel writes
    past the array: because the walker reads back exactly the bytes the kernel
    wrote there, every assertion passes. That is an unkillable stack overflow,
    measured on `net/cmsg.rs` (`MAX_CTRL` 304 → 240 against a 280-byte cmsg).
    Write the layout as a `#[repr(C)]` struct, take `size_of`, and pin the safety
    direction where the compiler can check it:
    `const _: () = assert!(MAX_CTRL >= rights_space(MAX_FDS) + cred_space());`.
    The mutation family disappears with the operators, and a layout that shrinks
    becomes a compile error with a name on it.
21. **Give the loop's terminal answer its own error.** When a retry loop returns
    the last errno it saw, every guard mutant that swallows one errno and retries
    in unchanged process state returns a byte-identical answer — only the syscall
    count differs — so the whole guard family is equivalent *at once*. Name the
    terminal condition (`SyscallError::E2BIG("getcwd")` past the allocation cap)
    and merge duplicated attempt arms into one loop with one guard, so each guard
    mutant changes the answer: four recorded equivalents in `env/getcwd.rs` died
    together that way (the deleted-CWD row goes red on the guard → `true` and on
    `==`→`!=`, the over-cap row on the guard → `false` and on `||`→`&&`).
22. **Operators need a site.** A cast and a method call generate no mutants, so
    derive a bound through them (`i64::MIN.unsigned_abs() as u128`,
    `u64::MAX as u128`) and share one `const` with every consumer of the number
    (rule 13) — the loop's break and the classifier's comparison then cannot
    drift. Do not launder a rule-bearing comparison into a method to silence a
    family (rule 16): a bound that must be exact belongs in a `const` with an
    assert (rule 20), and the boundary value's *variant* gets its own row.
23. **Clean the crate root before trusting a mutation probe, and never stage it.**
    An aborted (mutated) test run leaks hundreds of files into the crate root,
    which is the unit tests' CWD, and a leaked file named `0` flips
    `child::test::tests::file_tests_on_fd_vars` (`test -f 0`). A probe measured on
    that tree reports a "kill" that is pollution — one gate mutant was recorded as
    killed on 27 such failures and re-probed green on a clean tree. `git clean
    -fd` before measuring, and stage explicit paths: `git add -A` commits the
    leak, and the committed `safe/fdshell/0` then breaks the *clean* suite at
    every run. Resolution (task #476): the tests now leave the crate root clean
    even when a run is aborted; the anchored `safe/fdshell/` `.gitignore` entries
    for the artifact prefixes are the commit backstop, and the pre-probe habit
    above stands.

24. **When an entry's own proof shows an operand can never matter, delete the
    operand and the family dies with it.** Prove the operand dead by hand (rule 1),
    then write the value in a shape with no operator left to mutate: a plain literal
    for a product, an exact range for a ±1, a predicate for a boundary comparison,
    the callee's own gate for a duplicated conjunct. The accepted-equivalent
    entries this chore resolved (task #477) each landed on a form an existing
    rule already names — rules 3, 6, 11, 13, 15, 16, 18, 20, 22; this one is
    the pointer, not a new recipe. A deleted operator cannot be re-flagged by a
    future sweep, and the pins that named the surviving behaviour stay green on
    the simplified source.

25. **When every consumer *clamps* the value, pin it at the computation site.**
    An external agent (the kernel, a library) is told the value and acts on at
    most what it has: `copy_file_range` copies only the bytes the source really
    holds, so an over-counting `remaining` — `end - start` → `end + start` —
    hands it a bigger total and the copy is byte-identical, which is why the
    record called it "unkillable by construction" and said "don't add a seeking
    test". It is killable one level up: the test calls the function and asserts
    the exact count, on a nonzero offset (the difference) plus the zero row
    (rule 3). Measured on `copy_file_range.rs::remaining`: its whole `-` family
    — `+` and `/` — now fails
    `copy_file_range/tests.rs::remaining_counts_the_bytes_past_the_position`,
    so the `/` mutant dies on arithmetic, not on a debug divide-by-zero.

26. **A pin that asserts a whole vector — length included — kills mutants a
    slice-consuming consumer cannot see.** A consumer handed a *slice* never sees an
    entry appended past the window's end, so the mutant that appended it looked
    equivalent; assert the computed vector exactly in a test and both the length and
    the per-byte bits get measured. Measured on
    `parse/redirect/clobber.rs::quote_mask`: the exact-vector rows in
    `parse/redirect/clobber/tests.rs` (`single_quote_span_masks_its_bytes`,
    `span_masks_the_escape_and_the_quote_bytes`,
    `each_quote_arm_is_gated_by_the_other_state`,
    `escape_pair_never_leaves_a_toggle_behind`) now kill its `i < e`→`<=` bound and
    the escape-arm `!in_quote` guard together — two records this chore deleted, made
    obsolete by the mask split, died for free.

27. **Read the same number once.** When one arm computes the same arithmetic
    twice — once to ask, once to report — each copy is its own mutant site and
    only one of them is observable: the copy whose value reaches an assertion
    dies, the other survives because every probe that reaches it is decided by
    state the arithmetic cannot change. Hoist the sum into one local and the
    surviving site disappears. Measured on `segment.rs`'s stray-closer arm: the
    run-offset sum was read by `is_stray_closer` (whose own sum feeds only the
    recognition table's quote gate, whose answer for the raw-text words this
    byte-level path receives never depends on the offset) and by the
    `Segment::StrayCloser` it pushes; after the hoist one `+` is left and it is
    killed by the existing exact-offset pin
    (`segment/tests.rs::a_run_first_word_brace_at_the_top_level_reports_stray`),
    while the ask-side sum stays killable only through rule 2's direct probe
    (`segment/tests.rs::the_stray_decision_measures_the_word_where_its_bytes_lie`,
    a quote-removed text read at an offset inside a quote region).

28. **A scan-start offset mutant dies on a state-flipping byte inside the
    shifted window.** For an `i + 2`-style start, put a quote (or separator) byte
    in the bytes the honest scan has already consumed as part of the word just
    before the operator; the mutant starts the scan mid-quote and answers
    differently. Pin the difference at the scan's consumer, at bash's value.
    Measured on `cond.rs`'s `||` short-circuit `let after = i + 2;` (`+`→`-`),
    killed by `tests/run.rs::the_or_gate_skips_from_behind_the_operator_not_the_left_word`
    (left word `a";"`, verified by applying the mutant and re-running the pin).
