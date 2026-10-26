//! The `&>` family's attached target arms (task #329): the combined def is
//! one open shared by fd 1 (`export_to`) and fd 2 (`extra_to`), and the
//! `%var` fd-variable source is mask-gated on its own `%` byte — `&>"%v"`
//! and `&>>'%v'` are the **file** `%v` (bash parity), `&>%x` stays the fd
//! variable. The operator gate (`'&>'%v`) and the pidvar arms are pinned
//! unchanged: the mask rule is the operand's, never the operator's.

#![allow(clippy::unwrap_used)]
use super::{BgRedirectResult, parse_bg_redirect};
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use crate::redirect::{RedirectDirection, RedirectSource};

/// The `&>`-family result of token 1 of a probe line.
fn bg_of(line: &[u8]) -> Option<BgRedirectResult> {
    let tokens = tokenize_statement(line).unwrap();
    parse_bg_redirect(&tokens, 1).unwrap().map(|(bg, _n)| bg)
}

/// Every quoted `%` shape is the combined path word, mask and all.
#[test]
fn quoted_percent_target_is_the_combined_path_word() {
    for (line, dir) in [
        (b"cat &>\"%v\"".as_slice(), RedirectDirection::Write),
        (b"cat &>>\"%v\"", RedirectDirection::Append),
        (b"cat &>>'%v'", RedirectDirection::Append),
    ] {
        let bg = bg_of(line).unwrap_or_else(|| panic!("line={line:?} must parse"));
        assert_eq!(bg.redirects.len(), 1, "line={line:?}: one combined def");
        let def = bg.redirects.first().unwrap();
        assert_eq!(def.export_to, 1, "line={line:?}");
        assert_eq!(def.extra_to, Some(2), "line={line:?}");
        assert_eq!(def.direction, dir, "line={line:?}");
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
}

/// The unquoted arms keep the fd-variable source, and the operator and
/// pidvar arms are unchanged by the operand gate.
#[test]
fn the_unquoted_var_and_pidvar_arms_are_unchanged() {
    for (line, dir) in [
        (b"cat &>%x".as_slice(), RedirectDirection::Write),
        (b"cat &>>%x", RedirectDirection::Append),
    ] {
        let bg = bg_of(line).unwrap_or_else(|| panic!("line={line:?} must parse"));
        let def = bg.redirects.first().unwrap();
        assert_eq!(def.export_to, 1, "line={line:?}");
        assert_eq!(def.extra_to, Some(2), "line={line:?}");
        assert_eq!(def.direction, dir, "line={line:?}");
        match &def.source {
            RedirectSource::Var(name) => {
                assert_eq!(name.as_bytes().unwrap(), b"x", "line={line:?}")
            }
            s => panic!("line={line:?} must be a var, got {s:?}"),
        }
    }
    // The operator gate: a quoted `&>` is word data, no redirect, no job.
    assert!(bg_of(b"cat '&>'%v").is_none());
    // The pidvar arms: no redirections, the name, `bg_force` per the `|`.
    let bg = bg_of(b"cat &>&p").unwrap();
    assert!(bg.redirects.is_empty());
    assert_eq!(bg.pidvar.unwrap().as_bytes().unwrap(), b"p");
    assert!(!bg.bg_force);
    let bg = bg_of(b"cat &>|&p").unwrap();
    assert!(bg.redirects.is_empty());
    assert_eq!(bg.pidvar.unwrap().as_bytes().unwrap(), b"p");
    assert!(bg.bg_force);
}
