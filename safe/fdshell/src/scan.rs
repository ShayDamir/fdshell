/// What sits at position `i`: a comment, a separator, an async-`&` boundary,
/// or an ordinary char.
#[derive(Debug, PartialEq)]
pub(crate) enum Boundary {
    /// A `#` comment starts here (not inside quotes or backticks).
    Comment,
    /// A separator (`;`, newline, or end of line) that ends the current token.
    Separator,
    /// The plain `&` async operator (POSIX #3.9.4.1): a bare `&` at a word
    /// start that ends the statement and runs it asynchronously (task #326).
    Async,
    /// An ordinary character with no special meaning at the boundary level.
    Char,
}

/// Classify what sits at position `i`.
///
/// A comment counts only at the start of a word, outside quotes and backticks.
/// A separator only counts when not inside quotes, a single-quoted span,
/// backticks, or an open `$( )` substitution.
pub(crate) fn boundary(line: &[u8], i: usize, state: &ScanState) -> Boundary {
    let bare = !state.in_quote && !state.in_single && !state.in_backtick;
    if bare && !state.word_active && line.get(i) == Some(&b'#') {
        return Boundary::Comment;
    }
    let is_sep = i == line.len()
        || (bare && state.paren_depth == 0 && matches!(line.get(i), Some(&b';') | Some(&b'\n')));
    if is_sep {
        return Boundary::Separator;
    }
    if bare && state.paren_depth == 0 && bg::is_bg(line, i, state) {
        return Boundary::Async;
    }
    Boundary::Char
}

mod advance;
pub(crate) mod bg;
pub(crate) mod heredoc;
mod skip;
mod state;

pub(crate) use skip::{comment_end, skip_comment};
pub(crate) use state::{Quotes, ScanState};

#[cfg(test)]
mod tests;
