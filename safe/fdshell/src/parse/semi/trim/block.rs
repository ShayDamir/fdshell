//! The two **non-empty** validators of a body/condition slice: each owns its
//! own `ParseError` argument and the bash-rc-2 rationale for rejecting the
//! empty slice. The slicing rules they run first live in `trim.rs`.

use super::Token;
use super::{trim_body, trim_condition};
use crate::error::parse::ParseError;
use error_stack::{Report, ensure};

/// A block arm's body tokens: `tokens` with the `;`s bracketing it trimmed
/// ([`trim_body`]), `err` when nothing is left — bash rejects an empty body
/// (`if x; then; fi`, `while x; do; done`, `for i in a; do; done`: all rc 2,
/// and an empty `do` body makes the loop unbounded at run time). The
/// **function-definition** body (`f() { … }`, task #302) is the same grammar
/// slot — POSIX #2.9's `function_def` nests the group's `compound_list` — so it
/// calls this helper with its own variant; the group's own check
/// (`parse/group_block.rs`) is the same predicate written inline.
pub(crate) fn block_body<'a>(
    tokens: &'a [Token],
    line: &[u8],
    err: ParseError,
) -> Result<&'a [Token], Report<ParseError>> {
    let body = trim_body(tokens, line);
    ensure!(!body.is_empty(), err);
    Ok(body)
}

/// A block's **condition** tokens: [`trim_condition`] applied, `err` when
/// nothing is left. bash requires a non-empty `list` as an `if`/`elif`/
/// `while`/`until` condition (`if ; then …`, `elif ; then …`, `while ; do …`,
/// `until ; do …` and the `&`/newline spellings: all rc 2, measured), and an
/// empty condition is **true** at run time, so `while ; do …; done` is an
/// unbounded loop. The test is on the **trimmed** slice: the raw slice between
/// the keywords always carries POSIX's `;`, and the `&`-only slice (`if &
/// then`) is raw-non-empty / trimmed-empty. A `for` word list and a `case`
/// word are not conditions — `for i in; do …` iterates zero times (bash rc 0).
pub(crate) fn block_condition<'a>(
    tokens: &'a [Token],
    line: &[u8],
    err: ParseError,
) -> Result<&'a [Token], Report<ParseError>> {
    let cond = trim_condition(tokens, line);
    ensure!(!cond.is_empty(), err);
    Ok(cond)
}
