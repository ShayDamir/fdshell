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

/// A `&>`-family token: the combined redirect (one open shared by fd 1 and fd
/// 2, emitted as a single `RedirectDef::combined`) or a background job's pid
/// variable. Second field: tokens the operator
/// consumes (1 for the bare `&>` / `&>>`, which takes the next token as its
/// path operand, 0 for every attached form). The quoting gate covers every
/// arm — a fully quoted word, and any word with a quote byte in the
/// operator, is a plain argument (task #250's `fq` rule generalized). The
/// `%var` target arm reads the mask bit of its own `%` byte
/// (`redirect::fd_var_operand`): a quoted `%` is the file-name word (#329).
pub fn parse_bg_redirect(
    tokens: &[Token],
    i: usize,
) -> Result<Option<(BgRedirectResult, usize)>, Report<ParseError>> {
    let Some(token) = tokens.get(i) else {
        return Ok(None);
    };
    let (t, _start, _end, _fq, mask, _spans) = token;
    let Some(rest) = t.strip_prefix(b"&>") else {
        return Ok(None);
    };
    // The quoting gate: `&>` is 2 operator bytes, `&>>` / `&>&name` /
    // `&>|&name` are 3 (POSIX XCU 2.9.1 reads only unquoted operator bytes),
    // so a quote byte anywhere in the operator makes the word an argument.
    // One gate covers every arm below; `path_offset` in the path arms equals
    // `op_end` — the same index, one owner (LESSONS).
    let op_end =
        2 + usize::from(rest.starts_with(b">") || rest.starts_with(b"&") || rest.starts_with(b"|"));
    if crate::parse::redirect::quoted_bytes(mask, 0, op_end) {
        return Ok(None);
    }
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
    // `path_offset` is the gate's `op_end` for these arms: the index one
    // past the operator's last byte, the mask index of the operand's first
    // byte, and the offset `path_word::suffix` slices at (one owner, LESSONS).
    let path_offset = 2 + (rest_len - path.len());
    let source = if crate::parse::redirect::fd_var_operand(&path, mask, path_offset) {
        RedirectSource::Var(path.get(1..).ok_or(ParseError::Never)?)
    } else {
        // The path is the token suffix after `&>` (and `>`), with its mask and
        // its zero-byte quote spans sliced at the same offset (`path_word`):
        // a **quoted** `%` is an ordinary file-name word (`&>"%v"` is the file
        // `%v`, bash parity, task #329).
        crate::parse::redirect::path_word::suffix(token, path_offset)?
    };
    // One operator, one open: the single def carries fd 1 as `export_to` and
    // fd 2 as `extra_to`, and both are fed by the same opened description.
    let def = RedirectDef::combined(direction, source);
    Ok(Some((
        BgRedirectResult {
            redirects: vec![def],
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

/// Append `r` in the order it was written. POSIX #2.5 applies every
/// redirection of a command in the order written, so "the last entry for a fd
/// wins" is a **consequence** of that order, not a property of the table: a
/// second redirect to a fd is not an error, and the earlier ones still take
/// effect (their files are opened/truncated, and a failing earlier open
/// aborts the command).
///
/// Sorting the list by target fd is what the append replaces, because the
/// order of a command's defs is observable wherever two defs touch the same
/// fd: `cmd 2>&1 > f` must apply `2>&1` while fd 1 is still the shell's, and
/// `echo x 2>missing/dir >f` must fail before `f` is opened. See
/// LESSONS: "POSIX redirect order is observable". Block tails already
/// append (`parse/block_tail.rs`, `parse/group_block/tail.rs`), so the whole
/// crate keeps one ordering rule.
pub fn insert_redirect(redirects: &mut Vec<RedirectDef>, r: RedirectDef) {
    redirects.push(r);
}

#[cfg(test)]
mod tests;
