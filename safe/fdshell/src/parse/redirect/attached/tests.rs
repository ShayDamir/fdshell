//! Tests for the attached redirect form, above all its two gates: a token
//! whose first `<` is followed by another `<` is the here-doc/here-string
//! operator, never a path redirect — the gate stops `3<<T` from opening the
//! path `<T`, the operator's own second byte is not an operand byte. And a
//! **quoted** first operator byte (task #264, POSIX XCU 2.9.1) is word data,
//! never an operator: `2'>'g` is an argument, `2> "&" 1` opens the file `&1`.

#![allow(clippy::unwrap_used)]
use super::attached;
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectSource};

/// The attached form of the operator token (token 1) of a probe line.
fn attached_of(line: &[u8]) -> Option<RedirectDef> {
    let tokens = tokenize_statement(line).unwrap();
    attached(tokens.get(1)?).unwrap()
}

fn assert_path(line: &[u8], export_to: i32) {
    let def = attached_of(line).unwrap_or_else(|| panic!("line={:?} must parse", line));
    assert_eq!(def.export_to, export_to, "line={:?}", line);
    assert!(
        matches!(def.source, RedirectSource::Path { .. }),
        "line={:?}",
        line
    );
}

/// A `<` whose next byte is `<` is a here-doc/here-string operator: no path
/// redirect, and the token falls through to the word rule (`scan::heredoc::io`
/// claims the ones that are operators).
#[test]
fn chevron_shaped_tokens_are_not_path_redirects() {
    for line in [
        b"cat 3<<T".as_slice(),
        b"cat 3<<-T",
        b"cat 3<< T",
        b"cat 0<<T",
        b"cat <<T",
        b"cat 3<<<x",
        b"cat <<<x",
    ] {
        assert!(attached_of(line).is_none(), "line={:?}", line);
    }
}

/// The gate reads the operator byte, so every real redirect shape is
/// unchanged: the default fds, the explicit io-number, append, clobber,
/// read-write, and the dup and var sources.
#[test]
fn every_redirect_shape_still_parses() {
    assert_path(b"cat <f", 0);
    assert_path(b"cat 3<f", 3);
    assert_path(b"cat >f", 1);
    assert_path(b"cat 3>f", 3);
    assert_path(b"cat 3>>f", 3);
    assert_path(b"cat 3<>f", 3);
    assert_path(b"cat 3>|f", 3);
    let def = attached_of(b"cat 2>&1").unwrap();
    assert_eq!(def.export_to, 2);
    assert!(matches!(def.source, RedirectSource::Dup(1)));
    let def = attached_of(b"cat 3>%x").unwrap();
    assert_eq!(def.export_to, 3);
    assert!(matches!(def.source, RedirectSource::Var(_)));
}

/// The quoted-byte half of the gate (task #264): a token whose first
/// `>`/`<` byte is quoted is word data, not an operator — the same shape
/// rule as the `<<` gate one level up.
#[test]
fn quoted_first_operator_byte_is_not_an_attached_redirect() {
    for line in [
        b"cat 2'>'g".as_slice(),
        b"cat 1\">\"f",
        b"cat 1'>>'g",
        b"cat 1'<>'g",
        b"cat 2'>&1'",
        b"cat '<'f",
        b"cat '>|'f",
    ] {
        assert!(attached_of(line).is_none(), "line={:?}", line);
    }
}

/// A quoted `&` after the operator ends the dup arm, and the tail falls
/// through as the path operand — the file `&1` (bash parity, task #264).
#[test]
fn quoted_dup_ampersand_falls_through_to_the_path() {
    let def = attached_of(b"cat 2>\"&\"1").unwrap();
    assert_eq!(def.export_to, 2);
    match def.source {
        RedirectSource::Path { path, .. } => assert_eq!(path.as_bytes().unwrap(), b"&1"),
        ref s => panic!("quoted `&` must open the path, got {s:?}"),
    }
}
