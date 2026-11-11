// mutants: -C --lib -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -C --test -C block_redirect -- -E 'test(/^parse::block_tail::/) | binary(block_bodies) | binary(block_closers) | binary(block_conditions) | binary(block_redirect)'

//! The closer-tail family's **pipeline** arm (task #490): a statement that starts
//! with a keyword block and has the tokenizer's pipe token behind its closer —
//! `for x in 1; do false; done | cat` — is a pipeline whose first stage is that
//! block, so the statement chain hands the whole line to
//! [`crate::parse::pipeline`] instead of letting the block arm keep it.
//!
//! Before this arm the block parser claimed the statement and its
//! [`super`] walk **stopped** at the pipe without claiming it, so the tail was
//! silently dropped and the block ran as a bare block in the main shell: the
//! failing command met the shell's own live errexit instead of a stage's fork.
//! bash's "Set -e" clause exempts "any command in a pipeline but the last", and it
//! exempts the whole command running in the stage — the stage's own subshell
//! decides its own fate (a failing command kills the *stage*, not the shell, and
//! stops the stage's loop there), while the top shell only ever sees the **last**
//! stage's status. Both fall out of the pipeline the arm restores: the fork per
//! stage, the state copy the gate flag rides, the last child's status that
//! `pipeline/reap.rs` reports. Nothing here gates anything; it only lets the line
//! be a pipeline.
//!
//! The arm asks the same tail arms [`super::redirects`] asks — the path, here-doc,
//! here-string and `&>` family, in source order (one rule, four readers) — and
//! answers only for what they leave: the tokenizer's own pipe token (the ONE
//! predicate `parse::redirect::pipe_token_span`, task #261), so a quoted `"|"`, an
//! escaped `\|` and a `>|`-absorbed byte stay what the token layer says they are.
//! A word the walk cannot read stays [`super::word`]'s report, which is why the arm
//! is only ever asked for a block that parsed. And the closers it answers for are
//! the keyword blocks whose tail [`super`] walks: the `{ …; }` group and the
//! definer have their own tails, and their reading of a pipe behind the closer is
//! pinned (#186/#190).

use crate::error::parse::ParseError;
use crate::parse::block_tail::{bg, close::split_close, closer_index, heredoc};
use crate::parse::classify::parse_redirect;
use crate::parse::here_string::parse_here_string;
use crate::parse::pipe_token_span;
use crate::parse::{ParsedLine, Token};
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// The closer of a block whose tail this module's family walks — the closers
/// `crate::parse::dispatch` builds these arms with. `None` means the arm does not
/// answer, and the two blocks written with a brace closer are `None` on purpose:
/// the group and the definer have their own tails, and their reading of a pipe
/// behind the closer is pinned (#186/#190); the arithmetic `for` answers through
/// whichever body it parsed (task #394), so at this layer its closer is not one
/// keyword. Every other statement has no closer at all.
fn closer(parsed: &ParsedLine) -> Option<&[u8]> {
    match parsed {
        ParsedLine::If(_) => Some(b"fi"),
        ParsedLine::Case(_) => Some(b"esac"),
        ParsedLine::For(_) | ParsedLine::While(_) | ParsedLine::Until(_) | ParsedLine::Wait(_) => {
            Some(b"done")
        }
        _ => None,
    }
}

/// Whether the parsed block's statement is really a pipeline: the first token the
/// tail walk does not own, in source order, is the tokenizer's pipe token
/// ([`crate::parse::pipeline`]'s split). `raw` is the whole statement's tokens, so
/// the closer is located by this family's own rule ([`closer_index`]) — a pipe
/// written **inside** a block body is the block parser's own business and never
/// reaches the answer, and the arm is only ever asked after a successful parse.
pub(crate) fn starts_a_pipeline(
    parsed: &ParsedLine,
    raw: &[Token],
    text: &ScriptText,
) -> Result<bool, Report<ParseError>> {
    let Some(kw) = closer(parsed) else {
        return Ok(false);
    };
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let at = closer_index(raw, kw).ok_or(ParseError::Never)?;
    let tail = split_close(kw, raw.get(at..).ok_or(ParseError::Never)?);
    let mut i = 0;
    while i < tail.len() {
        let tok = tail.get(i).ok_or(ParseError::Never)?;
        if let Some((_, extra)) = heredoc::def_at(text, &tail, i)? {
            i += extra + 1;
            continue;
        }
        if let Some((_, extra)) = parse_here_string(line, &tail, i)? {
            i += extra + 1;
            continue;
        }
        if let Some((_, extra)) = bg::defs_at(&tail, i)? {
            i += extra + 1;
            continue;
        }
        if let Some((_, extra)) = parse_redirect(&tail, i)? {
            i += extra + 1;
            continue;
        }
        return Ok(pipe_token_span(line, tok.start, tok.end));
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
