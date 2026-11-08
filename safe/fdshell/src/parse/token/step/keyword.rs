// mutants: -C --lib -- -E 'test(/^parse::token::/)'

//! The keyword-word split of the unquoted-byte dispatcher (task #396): a
//! metacharacter byte ends the word **only when the bytes accumulated so far
//! are exactly a reserved word, ungated-quoted** — bash's tokenizer ends a
//! word at a metacharacter, and the one word-break set
//! ([`crate::bytes::word_break`]) is kept at both granularities by this arm
//! and the recognition boundary `keywords::kind::boundary`. Every other word
//! keeps fdshell's current segmentation (the general break is tasks
//! #316/#470/#422/#499), so the definer's glued-`(` token `foo(` is untouched.
//!
//! No separator token is pushed: after the split, the token stream for
//! `for((i=0` is exactly the one the spaced `for ((i=0` already produced.

use super::State;
use crate::bytes::word_break;
use crate::error::parse::ParseError;
use crate::keywords::reserved_word;
use error_stack::Report;

impl State {
    /// Whether the word-break byte `b` ends the word under construction: the
    /// word has started, its text is exactly a reserved word
    /// ([`crate::keywords::reserved`], the one list), and its raw span carries
    /// no quote/escape byte — `ShortCStr::is_shorter_than_span`, called not
    /// re-derived
    /// (POSIX §2.9.1: a quoted keyword word is data, and `'for'((…)`/`"for"((…)`
    /// /`\for((…)` stay command words).
    pub(super) fn keyword_run(&self, b: u8) -> bool {
        self.word_started
            && word_break(b)
            && self.cur.as_bytes().is_ok_and(reserved_word)
            && !self.cur.is_shorter_than_span(self.start, self.pos - 1)
    }

    /// End the keyword word at the metacharacter and let the metacharacter
    /// start the new word: **every branch that ends a word moves `start`
    /// too** (LESSONS "Token spans are the contract of the pre-parse
    /// rewrites"), and the span of the word the metacharacter starts covers
    /// the metacharacter itself.
    pub(super) fn split_keyword_run(&mut self, b: u8) -> Result<(), Report<ParseError>> {
        self.emit(self.pos - 1);
        self.word_reset();
        self.start = self.pos - 1;
        self.word_start();
        self.push_masked(b, false)
    }
}
