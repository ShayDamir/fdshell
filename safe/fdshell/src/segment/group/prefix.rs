//! The run tests a `{` is judged by, with the `time [-p]` prefix made
//! transparent: a prefix is a statement prefix, never a word of its own, so a
//! group written behind one is the same group.

use crate::keywords::time_prefix_len;

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

/// `x && time`, `x || time -p`: the `time` prefix trails a cond operator, so
/// the group behind it starts a cond part.
fn trails_cond_op(body: &[u8]) -> bool {
    let stem = body
        .strip_suffix(b" -p")
        .unwrap_or(body)
        .strip_suffix(b"time")
        .unwrap_or(body)
        .trim_ascii_end();
    ends_cond_op(stem)
}

#[cfg(test)]
mod tests;
