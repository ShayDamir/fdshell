#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

/// Every opener row of the table, with the closer each one requires. The
/// length-boundary rule (a prefix like `forx` is no keyword) is pinned at
/// `keywords/tests.rs`; here the **kind** is what a mutation must not flip.
#[test]
fn openers_carry_their_block_closer() {
    assert_eq!(
        keyword_kind(b"case", b"case x in", 4),
        Some(Keyword::Opens(Closer::Esac))
    );
    assert_eq!(
        keyword_kind(b"if", b"if :; then", 2),
        Some(Keyword::Opens(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"for", b"for i in a", 3),
        Some(Keyword::Opens(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"while", b"while :; do", 5),
        Some(Keyword::Opens(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"until", b"until :; do", 5),
        Some(Keyword::Opens(Closer::Done))
    );
    // The event-`wait` block opens a `done` block, the builtin opens nothing.
    assert_eq!(
        keyword_kind(b"wait", b"wait\n readable %rd) x ;;", 4),
        Some(Keyword::Opens(Closer::Done))
    );
    assert_eq!(keyword_kind(b"wait", b"wait 123", 4), None);
    // `{`/`}` pair with the brace, never with `done`: the group counter owns
    // them, and a stack that pairs them against a loop would close it.
    assert_eq!(
        keyword_kind(b"{", b"{ echo a; }", 1),
        Some(Keyword::Opens(Closer::Brace))
    );
    assert_eq!(
        keyword_kind(b"}", b"{ echo a; }", 11),
        Some(Keyword::Closes(Closer::Brace))
    );
}

/// Every closer row, and the glued redirect form (`fi>f`) that closes the block
/// and leaves the operator to `parse::block_tail` — with a **quoted** operand
/// too (task #413): the gate reads the keyword's own bytes, and the bytes behind
/// the word break are the operand's, exactly as bash splits the word.
#[test]
fn closers_carry_their_block_closer() {
    assert_eq!(
        keyword_kind(b"fi", b"fi; echo x", 2),
        Some(Keyword::Closes(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"fi>f", b"fi>f", 4),
        Some(Keyword::Closes(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"done", b"done; echo x", 4),
        Some(Keyword::Closes(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"done&", b"done&", 5),
        Some(Keyword::Closes(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"esac", b"esac; echo x", 4),
        Some(Keyword::Closes(Closer::Esac))
    );
    assert_eq!(
        keyword_kind(b"esac|", b"esac|", 5),
        Some(Keyword::Closes(Closer::Esac))
    );
    // Glued to a **quoted** operand the closer is still the closer (task #413):
    // the gate reads the keyword's own bytes, what sits behind the breaking byte
    // is the operand's, so the tail module takes the redirection — bash's split.
    assert_eq!(
        keyword_kind(b"done>\"f\"", b"done>\"f\"", 8),
        Some(Keyword::Closes(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"fi>\"f\"", b"fi>\"f\"", 6),
        Some(Keyword::Closes(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"esac>\"f\"", b"esac>\"f\"", 9),
        Some(Keyword::Closes(Closer::Esac))
    );
    assert_eq!(
        keyword_kind(b"done>>\"f\"", b"done>>\"f\"", 10),
        Some(Keyword::Closes(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"done>&\"f\"", b"done>&\"f\"", 10),
        Some(Keyword::Closes(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"done>&2", b"done>&2", 7),
        Some(Keyword::Closes(Closer::Done))
    );
    // The quotes must be the keyword's **own** bytes for the gate to fire: a
    // quoted keyword glued to an operand is a command word, not a closer.
    assert_eq!(keyword_kind(b"\"fi\">f", b"\"fi\">f", 6), None);
}

/// Openers glued to a quoted operand are scored by the same gate (task #413) —
/// one gate, both granularities, so the byte scan and the token scan cannot
/// disagree. What the whole shell then does is pinned e2e in
/// `safe/fdshell/tests/block_redirect.rs::a_glued_opener_with_a_quoted_operand_keeps_its_measured_answer`.
#[test]
fn a_glued_opener_with_a_quoted_operand_is_scored() {
    assert_eq!(
        keyword_kind(b"if>\"x\"", b"if>\"x\"; then :; fi", 6),
        Some(Keyword::Opens(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"while>\"x\"", b"while>\"x\"; do :; break; done", 10),
        Some(Keyword::Opens(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"case>\"x\"", b"case>\"x\" in x) :;; esac", 8),
        Some(Keyword::Opens(Closer::Esac))
    );
    assert_eq!(
        keyword_kind(b"for((\"x\")", b"for((\"x\") :; do :; done", 9),
        Some(Keyword::Opens(Closer::Done))
    );
    // Behind a `&&` the same spelling is an argument word with a redirection
    // (`a && if >x"y"` in bash), and the row's own boundary rule scores it.
    assert_eq!(
        keyword_kind(b"if>x\"y\"", b"a&&if>x\"y\"; then :; fi", 11),
        Some(Keyword::Opens(Closer::Fi))
    );
    // The gate still fires when the quote bytes are the keyword's own.
    assert_eq!(keyword_kind(b"\"if\"", b"\"if\"; then :; fi", 4), None);
}

/// The gate on the **token-level** reader's input (task #413, the shape that
/// regressed once): `parse::semi::closing` hands the table the quote-removed text and
/// the raw span end, so a fully quoted keyword word whose text carries a break byte
/// (`"done x"`, `"fi x"`, `"fi>y"`) is measured on the keyword's **own** bytes —
/// which lie inside the quote — and stays data, as POSIX §2.9.1 and bash read it.
/// The rows are pinned e2e in
/// `safe/fdshell/tests/block_redirect.rs::a_quoted_keyword_word_in_a_block_stays_data`.
/// The controls must not move: the same words in argument position, a quoted keyword
/// glued to an operand at the top level, and a quoted word that is no keyword at all.
#[test]
fn a_quoted_keyword_word_stays_data_at_the_gate() {
    assert_eq!(
        keyword_kind(
            b"done x",
            b"while :; do \"done x\"; echo b; break; done",
            20
        ),
        None
    );
    assert_eq!(
        keyword_kind(b"fi x", b"if :; then \"fi x\"; echo t; fi", 17),
        None
    );
    assert_eq!(
        keyword_kind(b"fi>y", b"if :; then \"fi>y\"; echo t; fi", 17),
        None
    );
    // Controls: the same word in argument position, and a quoted word that is no
    // keyword at all.
    assert_eq!(
        keyword_kind(b"done x", b"while :; do echo \"done x\"; break; done", 25),
        None
    );
    assert_eq!(
        keyword_kind(b"echo x", b"while :; do echo \"echo x\"; break; done", 25),
        None
    );
    // The top-level spelling of the glue: the row matches the text, the gate reads
    // the span, and bash runs the word.
    assert_eq!(keyword_kind(b"if>x", b"\"if>x\"; then echo i; fi", 6), None);
    assert_eq!(keyword_kind(b"fi>x", b"\"fi>x\"", 6), None);
}

/// The arm no reader reaches: a hand-written pin whose `word` and `word_end` disagree
/// is the one case `keywords/quoted.rs::span_start` cannot align (the pins at
/// `keywords/tests.rs::time_prefix_is_transparent_for_the_depth_rule` and
/// `keywords/tests.rs::keyword_delta_group_delimiters` live on it). Such a word is
/// decided by the row's literal-prefix rule — the keyword's own bytes are unquoted, so
/// the word is the keyword, exactly as the glued `a&&if>x"y"` row above.
#[test]
fn the_gate_of_an_unalignable_word_is_the_row() {
    assert_eq!(
        keyword_kind(b"fi>xq", b"fi>x\"y\"", 7),
        Some(Keyword::Closes(Closer::Fi))
    );
    // The same word aligned with the line: the answer is the same, and it comes from
    // the keyword's own bytes, not the operand's quotes.
    assert_eq!(
        keyword_kind(b"fi>xq", b"fi>xq", 5),
        Some(Keyword::Closes(Closer::Fi))
    );
}

/// A word that is no keyword is `None`, not a `0`-delta keyword: a quoted
/// keyword word is data, an ordinary word is data, and the `time [-p]` prefix
/// run is transparent — the keyword behind it carries the kind.
#[test]
fn non_keywords_and_the_time_prefix() {
    // A quoted keyword word is data (`quoted_word` reads the word span).
    assert_eq!(keyword_kind(b"\"if\"", b"\"if\"", 4), None);
    assert_eq!(keyword_kind(b"if", b"echo \"if\"", 9), None);
    // Ordinary words and keyword prefixes.
    assert_eq!(keyword_kind(b"ifconfig", b"ifconfig", 8), None);
    assert_eq!(keyword_kind(b"donec", b"donec", 5), None);
    assert_eq!(keyword_kind(b"echo", b"echo fi", 4), None);
    assert_eq!(keyword_kind(b"fiend", b"fiend", 5), None);
    // The `time [-p]` prefix run is stripped, including a repeated prefix.
    assert_eq!(
        keyword_kind(b"time if", b"time if :; then", 7),
        Some(Keyword::Opens(Closer::Fi))
    );
    assert_eq!(
        keyword_kind(b"time -p while", b"time -p while :; do", 13),
        Some(Keyword::Opens(Closer::Done))
    );
    assert_eq!(
        keyword_kind(b"time time -p fi", b"time time -p fi", 15),
        Some(Keyword::Closes(Closer::Fi))
    );
    // A bare `time` opens nothing of its own.
    assert_eq!(keyword_kind(b"time", b"time", 4), None);
    // The glued arithmetic `for` **is** a keyword word now (task #396): the `(`
    // ends the keyword word on the one word-break set.
    assert_eq!(
        keyword_kind(b"for((i=0", b"for((i=0;i<2;i++))", 8),
        Some(Keyword::Opens(Closer::Done))
    );
    // The arithmetic `for` whose region has a group opener behind it is **no
    // keyword** either (task #394): the brace pair nets zero and the group branch
    // takes the construct. Without an opener the word keeps its `done` frame.
    assert_eq!(
        keyword_kind(b"for", b"for ((i=0;i<2;i++)) { echo a; }", 3),
        None
    );
    assert_eq!(
        keyword_kind(b"for", b"for ((i=0;i<2;i++)) ; { echo a; }", 3),
        None
    );
    assert_eq!(
        keyword_kind(b"for", b"for ((i=0;i<2;i++)) do echo a; done", 3),
        Some(Keyword::Opens(Closer::Done))
    );
}
