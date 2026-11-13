#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::error::parse::ParseError;
use error_stack::Report;
use sys::ShortCStr;

fn word(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

fn parse(s: &str) -> Result<UmaskMode, Report<ParseError>> {
    UmaskMode::parse(&word(s))
}

fn fails_with(s: &str, pred: impl Fn(&ParseError) -> bool) -> bool {
    match parse(s) {
        Err(err) => pred(err.current_context()),
        Ok(mode) => panic!("{s:?} parsed as {mode:?}"),
    }
}

/// The digit-first rule is bash's (`DIGIT (*list->word->word)`), and the `0o` prefix
/// stays fdshell's extension (task #472): bash answers `octal number out of range`
/// for `0o22`, and fdshell sets the mask. `07777` is out of bash's octal range too
/// and stays fdshell's own call (task #472's decision).
#[test]
fn a_digit_first_word_is_octal() {
    for (word, want) in [
        ("022", 0o022),
        ("0o077", 0o077),
        ("0", 0o0),
        ("0o22", 0o022),
        ("07777", 0o7777),
    ] {
        assert_eq!(
            parse(word).unwrap(),
            UmaskMode::Octal(want),
            "word {word:?}"
        );
    }
}

#[test]
fn any_other_first_byte_is_symbolic() {
    for word in ["u=rwx", "a=rwx", "=x", "g-w", "u=Xg", "ug=r", "+", "=", "-"] {
        assert!(
            matches!(parse(word).unwrap(), UmaskMode::Symbolic(_)),
            "word {word:?}"
        );
    }
}

/// Only the first byte decides: `8` and `0o078` are octal words, and their error is
/// the octal one — bash's message says `octal number out of range`, and fdshell keeps
/// its own (task #472).
#[test]
fn the_digit_rule_looks_only_at_the_first_byte() {
    for word in ["8", "0o078", "9x"] {
        assert!(
            fails_with(word, |e| matches!(e, ParseError::InvalidOctal)),
            "word {word:?}"
        );
    }
}

/// A clause needs an operator right after its `who` letters; bash reports the byte it
/// found, the end of the word as the empty one, and fdshell renders the same.
#[test]
fn a_clause_needs_an_operator() {
    for (word, name) in [
        ("u", ""),
        ("ub", "b"),
        (",", ","),
        ("u=rwx,", ""),
        ("u=rwx,q", "q"),
        ("", ""),
    ] {
        assert!(
            fails_with(word, |e| matches!(e,
                ParseError::InvalidUmaskSymbolicOperator { op } if op.eq_bytes(name.as_bytes()))),
            "word {word:?}"
        );
    }
}

#[test]
fn an_action_run_ends_at_an_unknown_letter() {
    for (word, ch) in [("u+q", "q"), ("u=rwx?q", "?"), ("u=rw;q", ";")] {
        assert!(
            fails_with(word, |e| matches!(e,
                ParseError::InvalidUmaskSymbolicCharacter { ch: c } if c.eq_bytes(ch.as_bytes()))),
            "word {word:?}"
        );
    }
}
