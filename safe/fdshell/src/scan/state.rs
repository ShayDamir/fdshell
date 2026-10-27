//! The lexer state of a line being scanned, and the quote pair carried
//! across nested scans.

/// Lexer state tracked while scanning a script line.
///
/// Tracks double- and single-quote spans, backtick spans, and `$( )` command
/// substitution and `(( ))` arithmetic-command nesting.
pub(crate) struct ScanState {
    pub(crate) in_quote: bool,
    /// Inside a `'…'` span: every byte of it is literal data — no separator,
    /// comment, operator, keyword or escape pair is recognized inside it.
    pub(crate) in_single: bool,
    pub(crate) in_backtick: bool,
    /// Inside `$( … )` or an arithmetic command `(( … ))`.
    pub(crate) paren_depth: u32,
    /// A word char was just consumed, so a following `#` is data, not a comment.
    pub(crate) word_active: bool,
}

impl ScanState {
    /// Fresh state with no open quotes, backticks, or substitutions.
    pub(crate) fn new() -> Self {
        Self {
            in_quote: false,
            in_single: false,
            in_backtick: false,
            paren_depth: 0,
            word_active: false,
        }
    }

    /// The scan is at a byte where a new word can start: outside a quote, a
    /// backtick body, and `$( … )`, and no word is being read.
    pub(crate) fn at_word_start(&self) -> bool {
        !self.in_quote
            && !self.in_single
            && !self.in_backtick
            && self.paren_depth == 0
            && !self.word_active
    }

    /// The quote state carried by the scanner, for the callers that hand it
    /// across a nested scan ([`Quotes`]).
    pub(crate) fn quotes(&self) -> Quotes {
        Quotes {
            double: self.in_quote,
            single: self.in_single,
        }
    }

    /// Restore a carried quote state ([`Quotes::into`]).
    pub(crate) fn set_quotes(&mut self, quotes: Quotes) {
        self.in_quote = quotes.double;
        self.in_single = quotes.single;
    }
}

/// Both quote states together, carried by the block scanners that take the
/// caller's quote state as a parameter and hand it back (the brace block and
/// the closing-keyword scan). Keeping the pair in one value is what stops a
/// caller from carrying only the `"` half (task #141).
#[derive(Clone, Copy)]
pub(crate) struct Quotes {
    pub(crate) double: bool,
    pub(crate) single: bool,
}

impl Quotes {
    /// A scan that starts outside both quotes.
    pub(crate) fn new() -> Self {
        Self {
            double: false,
            single: false,
        }
    }

    /// Seed a fresh `ScanState` with this quote state.
    pub(crate) fn scan_state(self) -> ScanState {
        ScanState {
            in_quote: self.double,
            in_single: self.single,
            ..ScanState::new()
        }
    }
}
