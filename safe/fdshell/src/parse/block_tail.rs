// mutants: -C --lib -C --test -C block_bodies -C --test -C block_closers -C --test -C block_conditions -C --test -C block_redirect -- -E 'test(/^parse::block_tail::/) | binary(block_bodies) | binary(block_closers) | binary(block_conditions) | binary(block_redirect)'

//! The keyword block's **tail**: the redirections written after its closing
//! keyword (`while :; do :; done > f`) — path, here-doc ([`heredoc`], task #408),
//! here-string and the `&>` family ([`bg`], task #409) — and the **word** written
//! there — bash
//! demands a separator after a closing keyword, so a word is its syntax error
//! ([`word`], task #405). The closer's own separator (a `;`/newline, not the
//! keyword) is the block span's end, so a trailing redirection rides in
//! the block's own text and every builder's `up_to_closing` truncation drops it:
//! the tail is parsed **at** the closer, by this one module, exactly as
//! [`crate::parse::group_block`] does it for `{ …; }`.
//!
//! The walk's **stop** has two readers (task #490): [`word`] reports an ordinary
//! word, and [`pipe`] — the family's pipeline arm, asked by the statement chain
//! after a successful parse — takes the tokenizer's pipe token, so `for … done |
//! cat` is read as the pipeline it is and the block becomes the pipeline's first
//! stage.
//!
//! The `&>` arm takes the family's redirect arms (`&> f`, `&>>f`, the `%var`
//! target) and **refuses its pidvar arm** (`done &>&job`): at a closer bash
//! rejects the line, so the walk reports its `&` there instead of backgrounding
//! the block.

pub(crate) mod bg;
pub(crate) mod close;
pub(crate) mod heredoc;
pub(crate) mod pipe;
pub(crate) mod word;

use super::Token;
use crate::error::parse::ParseError;
use crate::parse::classify::parse_redirect;
use crate::parse::here_string::parse_here_string;
use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use close::split_close;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// The bytes a closing keyword glues to and stays a closer: the redirection
/// operator bytes (`done>f`, `esac>>out`). The byte-level closer boundary in
/// [`crate::keywords::keyword_delta`] accepts these plus the `&`/`|` glue
/// (`done&`, `done| cat`), which the tokenizer already splits into their own
/// tokens, so only the redirection pair is the tail's own.
const GLUE: &[u8] = b"<>";

/// Whether `tok` is the block's closing keyword `kw`: the bare word, or the word
/// glued to a redirection operator (the form [`redirects`] splits).
///
/// Stays free: it matches the word against this module's own `GLUE` set, not
/// only against the token's state (task #550).
pub(crate) fn is_closer(tok: &Token, kw: &[u8]) -> bool {
    let Token { word: t, .. } = tok;
    if t.eq_bytes(kw) {
        return true;
    }
    t.starts_with(kw)
        && t.as_bytes()
            .is_ok_and(|b| b.get(kw.len()).is_some_and(|&c| GLUE.contains(&c)))
}

/// The index of the block's closing keyword in `tokens`: the **last** token the
/// closer rule matches, since a keyword-shaped word inside a body (`echo done`)
/// is word data and the closer is the block's own last keyword.
pub(crate) fn closer_index(tokens: &[Token], kw: &[u8]) -> Option<usize> {
    (0..tokens.len()).rfind(|&i| tokens.get(i).is_some_and(|t| is_closer(t, kw)))
}

/// The redirections written after the closing keyword `kw`, in source order:
/// `Scope::open` needs the list stable per target fd, and last-wins per fd is
/// what bash gives (`done > f > g` leaves `f` empty, `g` holds the output).
///
/// The path-redirect arm is [`parse_redirect`]'s, the here-doc arm is
/// [`heredoc`]'s, the here-string arm is [`parse_here_string`]'s and the `&>`
/// arm is [`bg`]'s — each is
/// gated on its own raw-span shape rule, so the walk simply takes them **in
/// source order** and continues past them (task #408: a here-doc/here-string
/// written behind the closer belongs to the whole block, and the block's `text`
/// is what claims its body region from; task #409: the `&>` family belongs to
/// the whole block the same way). The order is not arbitrary and is not
/// load-bearing either: [`parse_redirect`] declines a `&`-prefixed token (its
/// prefix is not all digits) and [`bg`] fires only on a `&>`-prefixed one, so the
/// two never contend — the same order
/// `parse/command/collect_tail.rs` and `parse/group_block/tail.rs` keep, one rule
/// with three readers. What the walk does **not** take is a
/// word: bash requires a separator after a closing keyword, so a token none of
/// the arms reads goes to [`word::judge`], and only the shapes [`word`] names
/// stay accepted.
pub(crate) fn redirects(
    kw: &[u8],
    at_closer: &[Token],
    text: &ScriptText,
) -> Result<Vec<RedirectDef>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let tail = split_close(kw, at_closer);
    let mut redirects = Vec::new();
    let mut i = 0;
    while i < tail.len() {
        let tok = tail.get(i).ok_or(ParseError::Never)?;
        if let Some((def, extra)) = heredoc::def_at(text, &tail, i)? {
            redirects.push(def);
            i += extra + 1;
            continue;
        }
        if let Some((def, extra)) = parse_here_string(line, &tail, i)? {
            redirects.push(def);
            i += extra + 1;
            continue;
        }
        if let Some((defs, extra)) = bg::defs_at(&tail, i)? {
            redirects.extend(defs);
            i += extra + 1;
            continue;
        }
        let Some((def, extra)) = parse_redirect(&tail, i)? else {
            word::judge(kw, tok, line)?;
            break;
        };
        redirects.push(def);
        i += extra + 1;
    }
    Ok(redirects)
}

#[cfg(test)]
mod tests;
