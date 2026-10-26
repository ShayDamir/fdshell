//! Tests for the attached redirect form, above all its two gates: a token
//! whose first `<` is followed by another `<` is the here-doc/here-string
//! operator, never a path redirect — the gate stops `3<<T` from opening the
//! path `<T`, the operator's own second byte is not an operand byte. And a
//! **quoted** first operator byte (task #264, POSIX XCU 2.9.1) is word data,
//! never an operator: `2'>'g` is an argument, `2> "&" 1` opens the file `&1`.

#![allow(clippy::unwrap_used)]
use super::attached;
use crate::mask::MaskBit;
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

/// The attached `%var` target is the same quoting rule on the **operand's**
/// first byte (task #329, POSIX #4.1): the fd-variable arm fires only while
/// the `%` is **unquoted**; a quoted `%` falls through to the plain file word
/// that bash opens. The mask asserts pin the **offset** the gate consults —
/// the `path_offset` `path_word::suffix` slices at — so an off-by-one gate
/// cannot pass: `>%v"w"` keeps the unquoted `%` (a gate reading the byte
/// *after* it sees the quoted `w` and wrongly yields a path), and `>"%v"x`
/// reads the quoted `%` (a gate reading the operator byte, index 0, wrongly
/// yields a var). The `>&%v` dup arm stays **mask-blind**: bash gives
/// `ambiguous redirect` rc 1 quoted *and* unquoted, so no file-name escape
/// exists there (§the task's measurement; pinned in `tests/redirect.rs`).
#[test]
fn quoted_percent_target_is_a_path_not_an_fd_variable() {
    for line in [
        b"cat >\"%v\"".as_slice(),
        b"cat >'%v'",
        b"cat >|\"%v\"",
        b"cat >>\"%v\"",
        b"cat <\"%v\"",
        b"cat 3>\"%v\"",
    ] {
        let def = attached_of(line).unwrap_or_else(|| panic!("line={line:?} must parse"));
        match &def.source {
            RedirectSource::Path { path, mask, .. } => {
                assert_eq!(path.as_bytes().unwrap(), b"%v", "line={line:?}");
                assert_eq!(mask.len(), 2, "line={line:?}: the mask rides the word");
                assert!(
                    mask.first().copied().is_some_and(MaskBit::quoted),
                    "line={line:?}: the `%` bit must be quoted"
                );
            }
            s => panic!("line={line:?} must be a path, got {s:?}"),
        }
    }
    // A `\%` needs no arm: the token text starts with `\`, so the text rule
    // never sees a `%` at the operand's start (fold strips the escape later).
    let def = attached_of(b"cat >\\%v").unwrap();
    match &def.source {
        RedirectSource::Path { path, mask, .. } => {
            assert_eq!(
                path.as_bytes().unwrap(),
                b"\\%v",
                "the text keeps the escape"
            );
            assert_eq!(mask.len(), 3, "line: the mask rides the word");
        }
        s => panic!("`>\\%v` must be a path, got {s:?}"),
    }
    // The unquoted arms keep today's parse-time reading, name = whole tail.
    for (line, name) in [
        (b"cat >%x".as_slice(), b"x".as_slice()),
        (b"cat 3>%x".as_slice(), b"x".as_slice()),
        (b"cat >|%x".as_slice(), b"x".as_slice()),
        (b"cat 3<%x".as_slice(), b"x".as_slice()),
        (b"cat >%v\"w\"".as_slice(), b"vw".as_slice()),
    ] {
        let def = attached_of(line).unwrap_or_else(|| panic!("line={line:?} must parse"));
        match &def.source {
            RedirectSource::Var(got) => {
                assert_eq!(got.as_bytes().unwrap(), name, "line={line:?}")
            }
            s => panic!("line={line:?} must be a var, got {s:?}"),
        }
    }
    // The off-by-one killer, other direction: the `%` at the operand index is
    // quoted, so the word is a path even though a later byte is unquoted.
    let def = attached_of(b"cat >\"%v\"x").unwrap();
    match &def.source {
        RedirectSource::Path { path, mask, .. } => {
            assert_eq!(path.as_bytes().unwrap(), b"%vx");
            assert!(
                mask.first().copied().is_some_and(MaskBit::quoted),
                "the gate reads the operand byte, not the operator byte"
            );
        }
        s => panic!("`>\"%v\"x` must be a path, got {s:?}"),
    }
    // The `>>` arm rejects the *unquoted* var target; the quoted one appends.
    let tokens = tokenize_statement(b"cat >>%x").unwrap();
    assert!(
        attached(tokens.get(1).unwrap()).is_err(),
        "`>>%x` stays the parse error it is"
    );
    // The dup arm stays mask-blind (bash: `ambiguous redirect` for both).
    for line in [b"cat 2>&\"%v\"".as_slice(), b"cat 2>&%v"] {
        let def = attached_of(line).unwrap();
        assert_eq!(def.export_to, 2, "line={line:?}");
        match &def.source {
            RedirectSource::Var(name) => assert_eq!(name.as_bytes().unwrap(), b"v"),
            s => panic!("line={line:?}: the dup arm stays mask-blind, got {s:?}"),
        }
    }
}
