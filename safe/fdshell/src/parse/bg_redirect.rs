mod bare;

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, ensure};
use sys::ShortCStr;

pub struct BgRedirectResult {
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

/// A `&>`-family token: the combined redirect (the same target on fd 1 and fd
/// 2) or a background job's pid variable. Second field: tokens the operator
/// consumes (1 for the bare `&>` / `&>>`, which takes the next token as its
/// path operand, 0 for every attached form).
pub fn parse_bg_redirect(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(BgRedirectResult, usize)>, Report<ParseError>> {
    // `_fq` (fully quoted) is read for task #250's quoted-word guard.
    let Some((t, _start, _end, _fq, mask, _spans)) = tokens.get(i) else {
        return Ok(None);
    };
    let Some(rest) = t.strip_prefix(b"&>") else {
        return Ok(None);
    };
    if let Some(name) = rest.strip_prefix(b"|&") {
        return Ok(Some(pidvar(name, true)));
    }
    if let Some(name) = rest.strip_prefix(b"&") {
        return Ok(Some(pidvar(name, false)));
    }
    // `&>|f` is not a background redirect: a `|` after `&>` is the clobber
    // operator's byte absorbed into this token, and bash rejects it as a
    // syntax error, so the `|` tail is rejected here.
    ensure!(!rest.starts_with(b"|"), ParseError::InvalidRedirect);
    let rest_len = rest.len();
    let (path, direction) = if let Some(p) = rest.strip_prefix(b">") {
        (p, RedirectDirection::Append)
    } else {
        (rest, RedirectDirection::Write)
    };
    // `&>` and `&>>` with nothing after the operator are bare: like the bare
    // `>` family, they take the next token as their operand.
    if path.is_empty() {
        return bare::parse(tokens, i, direction).map(|bg| Some((bg, 1)));
    }
    let source = if path.starts_with(b"%") {
        RedirectSource::Var(path.get(1..).ok_or(ParseError::Never)?)
    } else {
        // The path is the token suffix after `&>` (and `>`); align its mask.
        let path_offset = 2 + (rest_len - path.len());
        let path_mask = mask.get(path_offset..).unwrap_or(&[]).to_vec();
        RedirectSource::path(path, path_mask)
    };
    let r1 = RedirectDef {
        export_to: 1,
        direction,
        source: source.clone(),
    };
    let r2 = RedirectDef {
        export_to: 2,
        direction,
        source,
    };
    Ok(Some((
        BgRedirectResult {
            redirects: vec![r1, r2],
            pidvar: None,
            bg_force: false,
        },
        0,
    )))
}

/// The `&>&name` / `&>|&name` pidvar form: no redirections, the job's pid
/// variable, and `bg_force` for the `|` variant. Consumes no following token.
fn pidvar(name: ShortCStr, bg_force: bool) -> (BgRedirectResult, usize) {
    (
        BgRedirectResult {
            redirects: Vec::new(),
            pidvar: Some(name),
            bg_force,
        },
        0,
    )
}

/// Insert `r` keeping the list sorted by target fd, and the entries of one
/// target fd in command-line order. POSIX #2.5 applies every redirection of a
/// command in the order written, so a second redirect to a fd is not an error:
/// the last one to that fd wins, and the earlier ones still take effect (their
/// files are opened/truncated, and a failing earlier open aborts the command).
/// `partition_point` is the stable insertion point; `binary_search_by_key`
/// returns an arbitrary equal-key index, which cannot express "last wins".
pub fn insert_redirect(redirects: &mut Vec<RedirectDef>, r: RedirectDef) {
    let at = redirects.partition_point(|x| x.export_to <= r.export_to);
    redirects.insert(at, r);
}
