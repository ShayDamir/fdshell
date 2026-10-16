//! The run tests a `{` is judged by, with the `time [-p]` prefix made
//! transparent: a prefix is a statement prefix, never a word of its own, so a
//! group written behind one is the same group.

use crate::keywords::{definer_len, time_prefix_len};

/// What the `{` at `brace` is, seen from the run that starts at `start` (the
/// byte after the last separator): `Some(true)` starts a new statement (the run
/// before it is empty), so the group is a block; `Some(false)` starts a cond
/// part mid-run (the run ends with `&&` / `||`), so the group's text is opaque
/// data inside the statement; `None` is command word data — a brace in an
/// argument position (`echo {`, `echo x{`), after `|` (a pipeline stage), or
/// glued to a word, as in bash.
///
/// A `time [-p]` prefix is transparent: `time { …; }` is a group statement and
/// `x && time { …; }` a group cond part, exactly like the unprefixed forms.
pub(crate) fn group_at_run(line: &[u8], start: usize, brace: usize) -> Option<bool> {
    let body = strip_prefix(line.get(start..brace).unwrap_or(b"").trim_ascii());
    if body.is_empty() {
        Some(true)
    } else if ends_cond_op(body) || trails_cond_op(body) {
        Some(false)
    } else {
        None
    }
}

/// The span start of a group statement: the `{` at `brace`, or a `time [-p]`
/// prefix written alone in the run before it (`time { …; }`) — the prefix must
/// sit inside the span so the `time` parse hook sees it.
pub(super) fn span_start(line: &[u8], run_start: usize, brace: usize) -> usize {
    let raw = line.get(run_start..brace).unwrap_or(b"");
    let trimmed = raw.trim_ascii();
    match time_prefix_len(trimmed) {
        Some(len) if len == trimmed.len() => run_start + (raw.len() - raw.trim_ascii_start().len()),
        _ => brace,
    }
}

/// The run's text with its `time [-p]` prefix run removed.
fn strip_prefix(raw: &[u8]) -> &[u8] {
    time_prefix_len(raw)
        .and_then(|p| raw.get(p..))
        .unwrap_or(raw)
        .trim_ascii()
}

/// The run ends with a cond operator: the group is a cond part.
fn ends_cond_op(body: &[u8]) -> bool {
    body.ends_with(b"&&") || body.ends_with(b"||")
}

/// `x && time`, `x || time -p`, `x && time time -p`: the prefix run trails a
/// cond operator, so the group behind it starts a cond part. The run is stripped
/// with the same rule [`strip_prefix`] uses on the statement side — a prefix is
/// transparent in every scorer of the run's first word, so a repeated prefix
/// cannot split the group (`x && time time -p { …; }`).
fn trails_cond_op(body: &[u8]) -> bool {
    ends_cond_op(strip_trailing_prefix(body))
}

/// The `{` at `brace` opens a function definition's body: the run before it is
/// exactly the definer words (`f() {`, `function f {`, `function f() {`) with
/// only `(` `)` whitespace as filler (`function f ( ) {`, `f() ( ) {`), and it
/// is the whole run. `f ( ) {` (POSIX split parens) answers `false`: the
/// definer rule rejects it, and the cond layer must not hide its operators.
///
/// The definer is the `time [-p]` transparency's twin — its prefix sits in the
/// span, keeping the definition one statement — but it is deliberately *not*
/// folded into [`group_at_run`]: `Some(true)` there would flush the definer as
/// a statement before the brace and the definition would never be claimed.
/// The segment layer asks "may I flush what precedes the brace" (no), the cond
/// layer asks "is this brace's text opaque" (yes), so only
/// `cond/boundary::opens_group` consumes this rule (task #235).
pub(crate) fn definer_at_run(line: &[u8], start: usize, brace: usize) -> bool {
    let body = line.get(start..brace).unwrap_or(b"").trim_ascii();
    opens_definer(body) || trails_definer(body)
}

/// The run text is the definer words plus `(`/`)`/whitespace filler and
/// nothing else. An empty run is not a definer (`definer_len(b"")` is `None`),
/// so a plain `{ …; }` keeps [`group_at_run`]'s answer. A *word* filler
/// (`f() echo { a && b; }`) never joins a definer run: bash rejects it with a
/// syntax error and fdshell keeps that error visible.
fn opens_definer(text: &[u8]) -> bool {
    definer_len(text).is_some_and(|len| text.get(len..).unwrap_or(b"").iter().all(filler))
}

/// Filler bytes allowed behind the definer words, permissive exactly as far
/// as the parse layer goes (`f() ( ) { … }` is accepted there, LESSONS "Byte-
/// level definer words must be the tokenizer's tokens").
fn filler(b: &u8) -> bool {
    matches!(b, b'(' | b')') || b.is_ascii_whitespace()
}

/// `true && f()`, `false || function f`: the run's tail after its last `&&` /
/// `||` is a definer run — the mid-run twin of [`trails_cond_op`]'s
/// `x && time { …; }`. The scan is quote-blind as `ends_cond_op` is: the
/// cond layer reaches this text with its regions already skipped.
fn trails_definer(text: &[u8]) -> bool {
    last_cond_op(text).is_some_and(|end| opens_definer(text.get(end..).unwrap_or(b"")))
}

/// The byte just past the run's last `&&` / `||`, `None` when it has none.
fn last_cond_op(text: &[u8]) -> Option<usize> {
    (0..text.len())
        .rfind(|&p| matches!(text.get(p..p + 2), Some(b"&&") | Some(b"||")))
        .map(|p| p + 2)
}

/// `body` without its trailing `time` / `-p` prefix words (the whitespace
/// between them goes with the words).
fn strip_trailing_prefix(body: &[u8]) -> &[u8] {
    let mut rest = body.trim_ascii_end();
    while let Some(shorter) = rest
        .strip_suffix(b"-p")
        .or_else(|| rest.strip_suffix(b"time"))
    {
        rest = shorter.trim_ascii_end();
    }
    rest
}

#[cfg(test)]
mod tests;
