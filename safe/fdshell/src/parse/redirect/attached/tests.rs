//! Tests for the attached redirect form, above all the `<<` gate: a token
//! whose first `<` is followed by another `<` is the here-doc/here-string
//! operator, never a path redirect. The gate is what stops `3<<T` from opening
//! the path `<T` — the operator's own second byte is not an operand byte.

#![allow(clippy::unwrap_used)]
use super::attached;
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDef, RedirectSource};

/// The attached form of the operator token (token 1) of a probe line.
fn attached_of(line: &[u8]) -> Option<RedirectDef> {
    let tokens = tokenize_statement(line).unwrap();
    let (t, _s, _e, _fq, mask, _spans) = tokens.get(1)?;
    attached(t, mask).unwrap()
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
