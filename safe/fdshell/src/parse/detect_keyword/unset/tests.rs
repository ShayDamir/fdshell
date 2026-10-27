#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;

/// `detect` on a whole statement: `unset`'s operand list.
fn targets(line: &[u8]) -> Vec<UnsetTarget> {
    let tokens = tokenize_statement(line).unwrap();
    match detect(&tokens) {
        ParsedLine::Unset(targets) => targets,
        _ => panic!("expected ParsedLine::Unset for {line:?}"),
    }
}

/// Each target as `(kind, name)`: `b'v'` a shell variable, `b'f'` an fd
/// variable, `b'a'` an array entry spelled `arr%src`.
fn shape(list: &[UnsetTarget]) -> Vec<(u8, ShortCStr)> {
    let sep = ShortCStr::from(c"%");
    list.iter()
        .map(|t| match t {
            UnsetTarget::Var(name) => (b'v', name.clone()),
            UnsetTarget::Fd(name) => (b'f', name.clone()),
            UnsetTarget::FdArrayEntry { var, source } => {
                (b'a', ShortCStr::concat(&[var, &sep, source]))
            }
        })
        .collect()
}

#[test]
fn a_plain_word_is_a_shell_variable() {
    assert_eq!(shape(&targets(b"unset v")), vec![(b'v', c"v".into())]);
}

#[test]
fn several_operands_keep_their_order() {
    assert_eq!(
        shape(&targets(b"unset a b c")),
        vec![
            (b'v', c"a".into()),
            (b'v', c"b".into()),
            (b'v', c"c".into())
        ]
    );
}

#[test]
fn a_percent_word_is_an_fd_variable() {
    assert_eq!(
        shape(&targets(b"unset %client")),
        vec![(b'f', c"client".into())]
    );
}

#[test]
fn an_array_entry_is_its_own_target() {
    assert_eq!(
        shape(&targets(b"unset %arr[%conn]")),
        vec![(b'a', c"arr%conn".into())]
    );
}

#[test]
fn a_malformed_array_tail_stays_a_plain_fd_name() {
    assert_eq!(
        shape(&targets(b"unset %arr[conn]")),
        vec![(b'f', c"arr[conn]".into())]
    );
}

#[test]
fn mixed_spellings_are_targets_in_the_order_written() {
    assert_eq!(
        shape(&targets(b"unset v %f w")),
        vec![
            (b'v', c"v".into()),
            (b'f', c"f".into()),
            (b'v', c"w".into())
        ]
    );
}

#[test]
fn no_operand_is_an_empty_list() {
    assert!(targets(b"unset").is_empty());
}

#[test]
fn a_quoted_word_is_one_name_quotes_and_all() {
    assert_eq!(
        shape(&targets(b"unset \"v w\"")),
        vec![(b'v', c"v w".into())]
    );
    assert_eq!(shape(&targets(b"unset \"v\"")), vec![(b'v', c"v".into())]);
}

#[test]
fn every_word_bash_accepts_is_a_target_not_an_error() {
    // bash 5.3.9: `unset 1`, `unset -`, `unset 1abc`, `unset a-b`, `unset '$#'`,
    // `unset '*'`, `unset -x`, `unset ""` → rc 0, silent, no effect.
    for (line, name) in [
        (&b"unset 1"[..], c"1"),
        (b"unset -", c"-"),
        (b"unset 1abc", c"1abc"),
        (b"unset a-b", c"a-b"),
        (b"unset $#", c"$#"),
        (b"unset *", c"*"),
        (b"unset -v", c"-v"),
    ] {
        assert_eq!(
            shape(&targets(line)),
            vec![(b'v', name.into())],
            "line: {line:?}"
        );
    }
    assert_eq!(shape(&targets(b"unset \"\"")), vec![(b'v', c"".into())]);
}

#[test]
fn a_glued_word_is_not_the_keyword() {
    // `unsetx v` is one word, so `detect::detect` never reaches this detector;
    // reaching it with any token list means every word from 1 on is a target.
    assert_eq!(
        shape(&targets(b"unset unsetx v")),
        vec![(b'v', c"unsetx".into()), (b'v', c"v".into())]
    );
}

/// The quote bytes of the keyword word are **transparent** (task #335): the
/// detector reads the words after it as the tokenizer wrote them — quotes
/// removed — so a quoted `unset` yields exactly the unquoted row's operand
/// list. The site is deliberately ungated on POSIX §2.9.1: an `unset` *builtin*
/// would receive the **expanded** words, and `unset %f` would arrive as the
/// fd's number, closing nothing (POSIX_COMPATIBILITY.md, "Quoted keywords are
/// command words, quoted control keywords are not").
#[test]
fn the_quote_bytes_of_the_keyword_are_transparent() {
    assert_eq!(shape(&targets(b"'unset' %f")), vec![(b'f', c"f".into())]);
    assert_eq!(shape(&targets(b"'unset' zz")), vec![(b'v', c"zz".into())]);
    assert_eq!(
        shape(&targets(b"u\"nset\" v %f")),
        vec![(b'v', c"v".into()), (b'f', c"f".into())]
    );
    assert!(targets(b"'unset'").is_empty());
    // The text rule again: `un"set"` is the keyword word, and an operand quoted
    // as a whole is one name, quotes and all.
    assert_eq!(shape(&targets(b"un\"set\" v")), vec![(b'v', c"v".into())]);
    assert_eq!(
        shape(&targets(b"'unset' \"v w\"")),
        vec![(b'v', c"v w".into())]
    );
}
