//! The event-case gate of `parse/wait_block`: does a `wait` statement open a
//! block, or is it the POSIX builtin? Extracted from `parse/wait_block.rs` for
//! the line budget (task #298 added the closer's redirections).

use crate::parse::Token;
use crate::parse::word_quoted::span_word;
use sys::ScriptText;

/// Whether the `wait` tokens open an event-case block: the first non-`;`
/// token after `wait` is an unquoted word on a subsequent line, or a same-line
/// pattern keyword. A same-line pid/`$!`/name, a quoted word, or nothing is
/// the POSIX `wait` builtin (it falls through to a command). The `;`-skip is
/// load-bearing: a `wait` on its own line tokenizes the following newline as
/// a `;` separator before the arm's first word (`wait\n readable …` yields
/// `wait`, `;`, `readable`, …), so the loop is reachable (a same-line `;`
/// never arrives — the statement splitter claims it first).
pub(crate) fn is_block(tokens: &[Token], text: &ScriptText) -> bool {
    let Some((_wt, _ws, we, _wq, _wm, _spans)) = tokens.first() else {
        return false;
    };
    let mut pos = 1;
    while let Some(tok) = tokens.get(pos) {
        if !span_word(tok, b";") {
            break;
        }
        pos += 1;
    }
    let Some((t, ts, _te, quoted, _tm, _spans)) = tokens.get(pos) else {
        return false;
    };
    if *quoted {
        return false;
    }
    let Some(word) = t.as_bytes().ok() else {
        return false;
    };
    let crossed = text
        .as_bytes()
        .ok()
        .and_then(|b| b.get(*we..*ts))
        .is_some_and(|between| between.contains(&b'\n'));
    crossed || crate::keywords::is_wait_pattern_keyword(word)
}
