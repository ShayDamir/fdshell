use crate::mask::MaskBit::Quoted;
use crate::mask::MaskBit::Unquoted;

/// Rule 19: the *program* never reaches the guard with a quoted byte inside a
/// `~user` prefix — fdshell expands no `~user`, so `~ro"ot"/x` reads as a
/// literal `~` whichever way the guard answers. The guard's own answer is what
/// is pinned here (the word bytes are `~root/x`, the quoted pair's bytes are
/// the mask bits), with `spans` empty so the **mask** arm of the disjunction
/// decides: `&&` for `||` answers `false` and this row goes red.
#[test]
fn the_guard_answers_a_quoted_byte_inside_a_user_prefix() {
    // `~ro"ot"/x` — quoted bytes 3 and 4, inside the prefix `~root/`.
    assert!(super::prefix_quoted(
        b"~root/x",
        &[
            Unquoted, Unquoted, Unquoted, Quoted, Quoted, Unquoted, Unquoted
        ],
        &[]
    ));
    assert!(!super::leads(
        b"~root/x",
        &[
            Unquoted, Unquoted, Unquoted, Quoted, Quoted, Unquoted, Unquoted
        ],
        &[]
    ));
    // The same word with no quoted byte is the other side of the guard, so the
    // mask scan is measured on both values, not argued from the arm.
    assert!(!super::prefix_quoted(b"~root/x", &[Unquoted; 7], &[]));
    assert!(super::leads(b"~root/x", &[Unquoted; 7], &[]));
}

/// Both sides of the prefix end — the first `/` **inclusive** (task #281): a
/// quote on the `/` itself counts, a quote after it does not. The `/` test is
/// the first operand of the guard's `position`, so it is pinned from both
/// directions rather than from the e2e rows alone (rule 9).
#[test]
fn the_guard_reads_through_the_first_slash_inclusive() {
    // `~"/*"`-style: the terminating `/`'s own bit is quoted ⇒ prefix quoted.
    assert!(super::prefix_quoted(
        b"~/x",
        &[Unquoted, Quoted, Unquoted],
        &[]
    ));
    // `~/"x"`: the pair sits **after** the `/`, so the prefix stays unquoted and
    // the tilde expands (`$HOME/x`, measured in bash).
    assert!(!super::prefix_quoted(
        b"~/x",
        &[Unquoted, Unquoted, Quoted],
        &[]
    ));
    assert!(super::leads(b"~/x", &[Unquoted, Unquoted, Quoted], &[]));
}

/// Both sides of the zero-byte span test: a quote pair that contributes no byte
/// is inside the prefix when it closes **at** the end index (task #321) and
/// outside when it closes past it. `spans` is the arm that decides these rows,
/// so the disjunction's `||` is measured in both operand directions (rule 8).
#[test]
fn the_guard_counts_a_zero_byte_span_at_the_end_only() {
    // `~""/x` — text `~/x`, the pair closed at index 1, the `/` itself.
    assert!(super::prefix_quoted(
        b"~/x",
        &[Unquoted, Unquoted, Unquoted],
        &[1]
    ));
    // `~/""` — the pair is past the prefix's end, so the guard stays quiet.
    assert!(!super::prefix_quoted(
        b"~/x",
        &[Unquoted, Unquoted, Unquoted],
        &[3]
    ));
    // A span at index 0 with a clean mask: only the spans arm can answer `true`.
    assert!(super::prefix_quoted(
        b"~/x",
        &[Unquoted, Unquoted, Unquoted],
        &[0]
    ));
}
