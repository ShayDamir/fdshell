use sys::ShortCStr;

/// Whether the word's raw text (the byte span `(start, end)`) contained
/// quotes or escapes: the raw span is longer than the word's own bytes.
///
/// An empty quoted region contributes no bytes to the word (and its per-byte
/// mask), so "this word was quoted" is not representable in the mask — it has
/// to be derived from the span. The same test is true for escaped bytes
/// (`"a\"b"`), which is intended: bash keeps such a word whole when its
/// expansion is empty, too.
pub(crate) fn word_quoted(t: &ShortCStr, start: usize, end: usize) -> bool {
    end - start > t.len()
}

#[cfg(test)]
mod tests;
