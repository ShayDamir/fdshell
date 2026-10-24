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
/// and leaves the operator to `parse::block_tail`.
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
    // The glued arithmetic `for` is not a keyword word (task #396).
    assert_eq!(keyword_kind(b"for((i=0", b"for((i=0;i<2;i++))", 8), None);
}
