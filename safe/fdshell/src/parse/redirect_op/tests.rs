//! The operator-tail arms of `redirect_op`, above all the `>>` arm's two
//! directions (task #329): `>>%v` is the rejected append-to-fd-variable form,
//! `>>"%v"` appends to the **file** `%v` (bash parity, POSIX #4.1). The `%`
//! arm reads index **1** of `after_mask` — the operand's first byte is
//! `after_op[1]`, the pair the caller hands in — and a mask shorter than the
//! text pads to *unquoted*, keeping today's text rule (the house idiom).

#![allow(clippy::unwrap_used)]
use super::redirect_op;
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use crate::redirect::RedirectDirection;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Token 1 of `cat <attached>` as `(after_op, after_mask)`: the tail after the
/// operator's first byte and its mask slice — exactly the pair `attached`
/// hands to `redirect_op`.
fn tail(line: &[u8]) -> (ShortCStr, Vec<MaskBit>) {
    let tokens = tokenize_statement(line).unwrap();
    let (t, _start, _end, _fq, mask, _spans) = tokens.get(1).unwrap();
    (t.get(1..).unwrap(), mask.get(1..).unwrap_or(&[]).to_vec())
}

/// The `>>` arm's two directions and the empty tail, plus the pad-to-unquoted
/// row that keeps the text rule for a mask that cannot see the `%` byte.
#[test]
fn append_rejects_the_fd_var_target_only_while_unquoted() {
    let (after, mask) = tail(b"cat >>%v");
    assert!(
        redirect_op(b'>', after, &mask).is_err(),
        "`>>%v` stays the parse error it is"
    );
    let (after, mask) = tail(b"cat >>\"%v\"");
    let (path, dir) = redirect_op(b'>', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"%v");
    assert!(matches!(dir, RedirectDirection::Append));
    // The plain tails are unchanged, quoted operand included.
    let (after, mask) = tail(b"cat >>x");
    let (path, dir) = redirect_op(b'>', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"x");
    assert!(matches!(dir, RedirectDirection::Append));
    // The empty tail is the bare `>>` arm's input; rejected here too.
    let (after, mask) = tail(b"cat >>");
    assert!(redirect_op(b'>', after, &mask).is_err());
    // A mask that carries no bit for the operand's first byte pads to
    // unquoted: the text rule (the var arm fires), never pad-to-quoted.
    let (after, _) = tail(b"cat >>\"%v\"");
    assert!(redirect_op(b'>', after.clone(), &[MaskBit::Unquoted]).is_err());
    assert!(redirect_op(b'>', after, &[]).is_err());
    let (after, _) = tail(b"cat >>x");
    let (path, dir) = redirect_op(b'>', after, &[]).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"x");
    assert!(matches!(dir, RedirectDirection::Append));
}

/// The other arms are untouched by the #329 gate: `>|` carries its `%` tail
/// through as text (the attached layer's mask gate decides var vs path), a
/// quoted continuation byte ends the operator, and the `<>`/`<`/`>` bare
/// reads stand.
#[test]
fn the_other_operator_tails_are_unchanged() {
    let (after, mask) = tail(b"cat >|%v");
    let (path, dir) = redirect_op(b'>', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"%v");
    assert!(matches!(dir, RedirectDirection::Clobber));
    let (after, mask) = tail(b"cat >\"|\"x");
    let (path, dir) = redirect_op(b'>', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"|x");
    assert!(matches!(dir, RedirectDirection::Write));
    let (after, mask) = tail(b"cat <>%v");
    let (path, dir) = redirect_op(b'<', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"%v");
    assert!(matches!(dir, RedirectDirection::Rw));
    let (after, mask) = tail(b"cat <>");
    assert!(redirect_op(b'<', after, &mask).is_err());
    let (after, mask) = tail(b"cat <%v");
    let (path, dir) = redirect_op(b'<', after, &mask).unwrap();
    assert_eq!(path.as_bytes().unwrap(), b"%v");
    assert!(matches!(dir, RedirectDirection::Read));
}
