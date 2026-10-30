//! The definer shape the parse dispatch rejects (task #368): the production
//! matches the statement, the word sitting in its name slot carries a `$( … )`
//! region, and such a word is never a name. The shape is the accepted
//! definer's (`parse::function_block`); the name rule is [`carries_region`].

use crate::keywords::carries_region;
use crate::parse::Token;
use crate::parse::word_quoted::{gated_word, keyword_word, span_word};
use sys::ShortCStr;

/// The name-slot word of a definition whose production matches and whose word
/// carries a region: reported, not run. Words the production does not match are
/// simple commands and keep running their words, and a word the quote gate
/// rejects is word data (task #247), so it is reported nowhere.
pub(crate) fn definer_with_region<'a>(tokens: &'a [Token], line: &[u8]) -> Option<&'a ShortCStr> {
    let slot = name_slot(tokens, line)?;
    let region = carries_region(slot.0.as_bytes().ok()?);
    (gated_word(slot, line) && region).then_some(&slot.0)
}

/// The word sitting in the production's name slot: word 1 of the
/// `function name [(]` pair, word 0 of the POSIX `name( )` pair — the two pairs
/// `parse::function_block` accepts, brace token included. `None` = the statement
/// does not match the production.
fn name_slot<'a>(tokens: &'a [Token], line: &[u8]) -> Option<&'a Token> {
    let brace = tokens.iter().any(|tok| keyword_word(tok, line, b"{"));
    let (first, second) = (tokens.first()?, tokens.get(1)?);
    if keyword_word(first, line, b"function") {
        return brace.then_some(second);
    }
    (brace && first.0.ends_with(b"(") && span_word(second, b")")).then_some(first)
}
