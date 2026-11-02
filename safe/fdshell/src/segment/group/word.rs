/// The byte that lets a `{` be a group opener: whitespace, `&`, `|`.
fn is_opener_boundary(b: &u8) -> bool {
    b.is_ascii_whitespace() || *b == b'&' || *b == b'|'
}

/// The byte that lets a `}` close a group: whitespace, `;`, `&`, `|`, a redirect
/// operator byte (`}>out3`), an fd digit (`}2>err`).
fn is_closer_boundary(b: &u8) -> bool {
    b.is_ascii_whitespace() || b.is_ascii_digit() || matches!(b, b';' | b'&' | b'|' | b'>' | b'<')
}

/// A group opener word: `{` followed by whitespace, `&`, `|`, and not the end
/// of input. The glued forms stay ordinary words: `{echo`/`{a,b}`/`{1..5}`/`{}`
/// (a command name, brace expansion), and `{;` is the word `{;` (a
/// command-not-found), as bash's syntax error is there too.
///
/// The word form deliberately does **not** count end of input, unlike the byte
/// form [`opens_at`] (task #402): it is called for words in *any* position, and
/// bash keeps an argument brace a word (`echo {` prints `{`); the byte form's
/// callers all apply the command-position rule (`prefix::group_at_run`), where a
/// bare `{` written last **is** the group opener.
///
/// The same predicate is the block-depth `+1` for a `{` word in
/// `keywords::keyword_delta`, so the segment opener and the depth rule cannot
/// drift. `word_end` is the byte just past the first word, which is where the
/// `;` of the glued `{;` sits when the word itself is the bare `{`.
pub(crate) fn opens(word: &[u8], line: &[u8], word_end: usize) -> bool {
    word.starts_with(b"{")
        && word
            .get(1)
            .or_else(|| line.get(word_end))
            .is_some_and(is_opener_boundary)
}

/// The `{` byte at `i` in `line` opens a group: the byte after it is whitespace,
/// `&`, `|`, or there is none — end of input counts as a boundary here (a bare
/// `{` written last is the group opener in bash, task #402), and every caller
/// applies the command-position rule that keeps an argument brace a word
/// (`prefix::group_at_run`; the word form [`opens`] stays exact because it is
/// reached in any position). The scan can therefore start a group mid-line and
/// at the very end of the input.
pub(crate) fn opens_at(line: &[u8], i: usize) -> bool {
    line.get(i) == Some(&b'{') && line.get(i + 1).is_none_or(is_opener_boundary)
}

/// A group closer word: `}` followed by whitespace, `;`, `&`, `|`, a redirect
/// operator byte (`}>out3`), an fd digit (`}2>err`), or the end of the word. It
/// is the block-depth `-1` for a `}` word, so the glued `}` and a nested
/// group's `}` net out in `closing_keyword_index` and `comment::depth_delta`.
pub(crate) fn closes(word: &[u8]) -> bool {
    word.starts_with(b"}") && word.get(1).is_none_or(is_closer_boundary)
}

#[cfg(test)]
mod tests;
