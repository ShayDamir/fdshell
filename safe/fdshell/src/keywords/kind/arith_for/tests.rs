#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// The table row's question — what stands **behind** the `for` word, measured at the
/// word's end (task #394). A closed region with a group opener behind it is the
/// arithmetic `for`'s brace body; every other shape keeps the `done` frame.
#[test]
fn the_row_fires_only_for_a_region_followed_by_an_opener() {
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) { echo a; }", 3));
    // The single separator the parse layer's tail rule allows, spaced or glued.
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) ; { echo a; }", 3));
    assert!(opens_a_group(b"for ((i=0;i<2;i++)); { echo a; }", 3));
    // Two separators are bash's rc-2 pair; they keep the parse layer's `do` error.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) ;; { echo a; }", 3));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) ; ; { echo a; }", 3));
    // No opener behind the region: the head keeps its `done` frame.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) do echo a; done", 3));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) echo a", 3));
    // No whitespace gap is the glued head (task #396), and it rows too: bash
    // runs `for((…)) { …; }` with the group as its body (measured parity).
    assert!(opens_a_group(b"for((i=0;i<2;i++)) { echo a; }", 3));
    // A region that never closes hands the word back to the `done` frame.
    assert!(!opens_a_group(b"for ((i=0;i<2;i++ { echo a; }", 3));
    // Not a region at all.
    assert!(!opens_a_group(b"for i in a; do echo a; done", 3));
    // Task #402: the head with its opener at the very end of input rows — bash
    // buffers `for ((…)) {` under `>` (measured; at EOF it reports
    // `unexpected end of file from '{'`, and fdshell's EOF report of the same
    // shape is pinned in `tests/repl.rs`). An argument-position brace at the
    // end does not: it is word data, as bash prints `a` and `{` there.
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) {", 3));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) echo {", 3));
    // Task #518: whitespace runs and comment rows interleave in the gap (bash
    // runs them). The comment test is the shared gate, so the must-stay-false
    // rows (the parse layer's) answer false here too.
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) ;\n# c\n{ echo a; }", 3));
    assert!(opens_a_group(b"for ((i=0;i<2;i++))\n# c\n{ echo a; }", 3));
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) # c\n{ echo a; }", 3));
    assert!(opens_a_group(b"for ((i=0;i<2;i++))# c\n{ echo a; }", 3));
    assert!(opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n# c\n# d\n{ echo a; }",
        3
    ));
    assert!(opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n# \"c\n{ echo a; }",
        3
    ));
    assert!(opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n# `c\n{ echo a; }",
        3
    ));
    assert!(opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n# c {\n{ echo a; }",
        3
    ));
    assert!(opens_a_group(b"for ((i=0;i<2;i++)) ; # c\n{ echo a; }", 3));
    // The bash rc-2 pair: the `;` slot sits before the comment arm (load-bearing),
    // and the gate's word-start rule keeps quoted, escaped and glued-`#` rows
    // ordinary word rows.
    assert!(!opens_a_group(
        b"for ((i=0;i<2;i++))\n# c\n; { echo a; }",
        3
    ));
    assert!(!opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n\"#\"\n{ echo a; }",
        3
    ));
    assert!(!opens_a_group(
        b"for ((i=0;i<2;i++)) ;\n\\#c\n{ echo a; }",
        3
    ));
    assert!(!opens_a_group(
        b"for ((i=0;i<2;i++)) ;\nx# c\n{ echo a; }",
        3
    ));
    assert!(!opens_a_group(b"for ((i=0;i<2;i++)) ;\n#c { echo a; }", 3));
}

/// The run gate's question, from the whole run and the line behind it (task #394):
/// only a run that is nothing but the head, with the body's opener behind it, opens
/// the block. A plain `do … done` for must not answer true — the row owns it and the
/// `done` frame closes it (the shape the block-tail tests pin).
#[test]
fn the_run_gate_takes_only_a_head_whose_body_is_a_group() {
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) { echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) ; { echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)); { echo a; }",
        0
    ));
    // The `do … done` for: the run is the head, the body is not a group.
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) do echo a; done",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)); do echo a; done",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) ; echo a",
        0
    ));
    // A run carrying more than the head is not the gate's run.
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++)) ;; { echo a; }".as_slice(),
        b"for ((i=0;i<2;i++)) ;; { echo a; }",
        0
    ));
    // The glued head (task #396) is this gate's run too — bash runs the
    // glued `for((…)) { …; }` with the group as its body (measured parity).
    assert!(opens_a_block(
        b"for((i=0;i<2;i++))".as_slice(),
        b"for((i=0;i<2;i++)) { echo a; }",
        0
    ));
    // Task #402: a head whose group opener sits at the very end of input is
    // this gate's run with an unclosed body — bash buffers the shape. The
    // argument-position twin keeps the word data and bash's at-once answer.
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) {",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))",
        b"for ((i=0;i<2;i++)) echo {",
        0
    ));
    // Task #518: the gate's run with comment rows in the gap (bash runs them;
    // both entry points must agree — "one byte rule answers both sides of it").
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++)) ;".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n# c\n{ echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"for ((i=0;i<2;i++))".as_slice(),
        b"for ((i=0;i<2;i++))\n# c\n{ echo a; }",
        0
    ));
    // Bash's rc-2 rows stay rejecting at the gate too.
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++))".as_slice(),
        b"for ((i=0;i<2;i++))\n# c\n; { echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"for ((i=0;i<2;i++)) ;".as_slice(),
        b"for ((i=0;i<2;i++)) ;\n\"#\"\n{ echo a; }",
        0
    ));
    // Task #560: the cond arm — the run's tail after its last `&&` / `||` is the
    // head, and bash runs the shape (measured `a a` rc 0 on every row here).
    assert!(opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n{ echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n# c\n{ echo a; }",
        0
    ));
    assert!(opens_a_block(
        b"false || for ((i=0;i<2;i++)) ;",
        b"false || for ((i=0;i<2;i++)) ;\n{ echo a; }",
        0
    ));
    // Two cond operators: the **last** one owns the tail (the rfind step — the
    // first-operator reading leaves a `true &&` in the tail and loses this row).
    assert!(opens_a_block(
        b"true && true && for ((i=0;i<2;i++)) ;",
        b"true && true && for ((i=0;i<2;i++)) ;\n{ echo a; }",
        0
    ));
    // The glued head (task #396) rows on a cond run too (bash measures `a a`).
    assert!(opens_a_block(
        b"true && for((i=0;i<2;i++)) ;",
        b"true && for((i=0;i<2;i++)) ;\n# c\n{ echo a; }",
        0
    ));
    // The rc-2 family in cond position answers `false` at the arm — the negative
    // twins of every positive row above (the comment **before** the `;`, quoted,
    // escaped and glued-`#` rows, the comment that swallows the brace, `; ;`,
    // `;;`), and a run whose tail is not a head while the opener side is true.
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n# c\n; { echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n\"#\"\n{ echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n\\#c\n{ echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\nx# c\n{ echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;",
        b"true && for ((i=0;i<2;i++)) ;\n#c { echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ; ; { echo a; }",
        b"true && for ((i=0;i<2;i++)) ; ; { echo a; }",
        0
    ));
    assert!(!opens_a_block(
        b"true && for ((i=0;i<2;i++)) ;; do echo a; done",
        b"true && for ((i=0;i<2;i++)) ;; do echo a; done",
        0
    ));
    assert!(!opens_a_block(
        b"true && echo x",
        b"true && echo x\n{ echo a; }",
        0
    ));
}

/// The run test the group branch and the span start share, called when the caller is
/// already looking at a `{`: the whole text is the head and nothing else.
#[test]
fn the_run_test_wants_the_head_and_nothing_else() {
    assert!(is_run_head(b"for ((i=0;i<2;i++))"));
    assert!(is_run_head(b"for ((i = 0; i < 2; i ++ ))"));
    // Text past the region is body, not head — except the gap the byte rule
    // allows (task #518): the single `;` slot and comment rows, which the walk
    // consumes, so a run of head + slot is still the head and nothing else.
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) {"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) }"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)) echo a"));
    assert!(is_run_head(b"for ((i=0;i<2;i++)) ;"));
    // Task #518 (measured bash `a a` rc 0 for `for ((…)) ;` + `# c` + `{ …; }`):
    // a run of head + slot + comment row is the head with its gap consumed.
    assert!(is_run_head(b"for ((i=0;i<2;i++)) ;\n# c"));
    assert!(is_run_head(b"for ((i=0;i<2;i++))\n# c"));
    // The glued head (task #396) rows with the spaced one; a word that is not
    // this construct does not.
    assert!(is_run_head(b"for((i=0;i<2;i++))"));
    assert!(!is_run_head(b"for i in a"));
    assert!(!is_run_head(b"for ((i=0;i<2;i++)"));
    assert!(!is_run_head(b"while :; do"));
}
