#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::pipeline_pipe;

/// The pipeline-pipe test is the tokenizer's pipe token, and each arm needs its
/// own row: a plain `|` (a pipe, at a run start too), a `|` absorbed into a
/// `>`-terminated word (`>|`, `&>`, `%>` — not a pipe), a `|` inside quotes and
/// the tail of an escape pair (word content, not a pipe). Every row is the
/// answer the tokenizer gives for the same bytes; the byte-level heredoc scan
/// reads nothing else (`scan::heredoc::ops::precedes_pipe`).
#[test]
fn plain_pipe_bytes_are_pipes() {
    for (line, k) in [
        (b"a | b" as &[u8], 2usize),
        (b"a|b", 1),
        (b"| b", 0),
        (b"a >| b | c", 7),
        (b"cat \"a>\"| x", 8),
        (b"cat x\\>| x", 7),
        (b"x\\| | y", 4),
    ] {
        assert!(
            pipeline_pipe(line, k),
            "{:?} at {k}",
            core::str::from_utf8(line)
        );
    }
}

/// The `>`-terminated arms absorb their `|`: `clobber_pipe` is the same rule the
/// tokenizer applies to the word, so an absorbed `|` is never a pipe byte. A
/// quoted `>` never terminates a word, and an escape pair shields its tail, so
/// `x\>` and `"a>"` leave the `|` a pipe (`cat x\>| <<EOF` counts 0 at both
/// levels, `cat x\>a>| <<EOF` counts 1).
#[test]
fn absorbed_pipe_bytes_are_not_pipes() {
    for (line, k) in [
        (b"a >| b" as &[u8], 3usize),
        (b"cat >| x", 5),
        (b"cat %>>| x", 7),
        (b"cat &>| x", 6),
        (b"cat x\\>a>| x", 9),
        (b"cat \"a\\>\"x>| x", 11),
        (b"a>|b", 2),
    ] {
        assert!(
            !pipeline_pipe(line, k),
            "{:?} at {k}",
            core::str::from_utf8(line)
        );
    }
}

/// Quoted `|` bytes are word content: the tokenizer strips the quotes, so their
/// **text** is `|` and a token-text predicate reads `echo "|" <<EOF` as a
/// pipeline stage (the byte/token count divergence task #261 closes).
#[test]
fn quoted_pipe_bytes_are_word_content() {
    for (line, k) in [
        (b"\"|\"" as &[u8], 1usize),
        (b"'|'", 1),
        (b"a\"|\"b", 2),
        (b"a'|'b", 2),
        (b"a\"b|c\"", 3),
        (b"echo \"|\" <<EOF", 6),
    ] {
        assert!(
            !pipeline_pipe(line, k),
            "{:?} at {k}",
            core::str::from_utf8(line)
        );
    }
}

/// The tail of an unquoted escape pair is word content (`x\|` is one word),
/// while the byte a pair *lands on* is a fresh byte: `x\>|` is a pipe, `x\|` is
/// not. Inside a span the backslash is literal, so `'a\>'` shields nothing and
/// the `|` after it is judged by the absorption test alone.
#[test]
fn escape_pair_tails_are_not_pipes() {
    for (line, k) in [
        (b"x\\|" as &[u8], 2usize),
        (b"\\|", 1),
        (b"x\\| <<EOF", 2),
        (b"a x\\|b", 4),
    ] {
        assert!(
            !pipeline_pipe(line, k),
            "{:?} at {k}",
            core::str::from_utf8(line)
        );
    }
    assert!(pipeline_pipe(b"x\\>|", 3), "the pair lands on the `|`");
    assert!(
        pipeline_pipe(b"'a\\>'|x", 5),
        "the span's `\\` is data: the `|` is a pipe"
    );
}

/// A `|` with no word in front of it (a run that opens with the pipe) is a
/// pipe: `word_start` stops at 0, the walk lands on `k` and `clobber_pipe`'s
/// empty word is not `>`-terminated. `| <<EOF` counts 0 operators at both
/// levels because of this row.
#[test]
fn pipe_at_run_start_is_a_pipe() {
    assert!(pipeline_pipe(b"| <<EOF", 0));
    assert!(pipeline_pipe(b"; | <<EOF", 2));
    assert!(pipeline_pipe(b"\n| <<EOF", 1));
}
