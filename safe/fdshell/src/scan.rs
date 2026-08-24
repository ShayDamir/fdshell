/// Lexer state tracked while scanning a script line.
///
/// Tracks double-quote toggling, backtick spans, and `$( )` command
/// substitution nesting.
pub(crate) struct ScanState {
    pub(crate) in_quote: bool,
    pub(crate) in_backtick: bool,
    pub(crate) dollar_paren_depth: u32,
    /// A word char was just consumed, so a following `#` is data, not a comment.
    pub(crate) word_active: bool,
}

impl ScanState {
    /// Fresh state with no open quotes, backticks, or substitutions.
    pub(crate) fn new() -> Self {
        Self {
            in_quote: false,
            in_backtick: false,
            dollar_paren_depth: 0,
            word_active: false,
        }
    }
}

/// What sits at position `i`: a comment, a separator, or an ordinary char.
#[derive(Debug, PartialEq)]
pub(crate) enum Boundary {
    /// A `#` comment starts here (not inside quotes or backticks).
    Comment,
    /// A separator (`;`, newline, or end of line) that ends the current token.
    Separator,
    /// An ordinary character with no special meaning at the boundary level.
    Char,
}

/// Classify what sits at position `i`.
///
/// A comment counts only at the start of a word, outside quotes and backticks.
/// A separator only counts when not inside quotes, backticks, or an open
/// `$( )` substitution.
pub(crate) fn boundary(line: &[u8], i: usize, state: &ScanState) -> Boundary {
    if !state.in_quote && !state.in_backtick && !state.word_active && line.get(i) == Some(&b'#') {
        return Boundary::Comment;
    }
    let is_sep = i == line.len()
        || (!state.in_quote
            && !state.in_backtick
            && state.dollar_paren_depth == 0
            && matches!(line.get(i), Some(&b';') | Some(&b'\n')));
    if is_sep {
        return Boundary::Separator;
    }
    Boundary::Char
}

mod advance;

#[cfg(test)]
mod tests;
