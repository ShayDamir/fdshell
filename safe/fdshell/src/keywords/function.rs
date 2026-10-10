//! The function-definition opener rule, shared by the byte-level segment
//! scanner (`brace::scan_function_block`) and the token-level parse dispatch
//! (`parse::function_block`), so the two layers accept the same shapes.

/// The definer name length in a definer word: non-empty, no whitespace, `{`,
/// `}` or `)`, and no `(` except as the last byte (the POSIX opener's `(`,
/// whose `)` is the next word). `None` = not a definer word.
pub(crate) fn name_len(word: &[u8]) -> Option<usize> {
    let open = word.iter().position(|&b| b == b'(');
    if open.is_some_and(|i| word.get(i + 1).is_some()) {
        return None;
    }
    let len = open.unwrap_or(word.len());
    if len == 0
        || word
            .iter()
            .any(|&b| b.is_ascii_whitespace() || matches!(b, b'{' | b'}' | b')'))
    {
        return None;
    }
    Some(len)
}

/// `word` opens a function block: the POSIX `name() {` form, or the bash/ksh
/// `function name [()] {` form. Returns the definer name.
///
/// `word` is the definer text as the segment scanner sees it, and its word
/// split is the tokenizer's (`)` is the separator token), so the definer words
/// here are exactly the definer tokens of the same text.
pub(crate) fn function_def_name(word: &[u8]) -> Option<&[u8]> {
    let (head, second) = def_words(word);
    if head.is_some_and(|w| w == b"function") {
        return second.and_then(name);
    }
    head.filter(|w| w.ends_with(b"(")).and_then(name)
}

/// The definer word with its trailing POSIX `(` stripped.
fn name(word: &[u8]) -> Option<&[u8]> {
    word.get(..name_len(word)?)
}

/// The first two definer words of `word`, split at whitespace and `)`.
fn def_words(word: &[u8]) -> (Option<&[u8]>, Option<&[u8]>) {
    let (first, rest) = skip_word(word);
    (first, skip_word(rest).0)
}

/// The first definer word, and the text after it. A `)` is the tokenizer's own
/// separator token, so it is a word of its own: it never joins the word before
/// it, and it is never consumed as a terminator, so a `)` glued after a word is
/// the start of the text returned with it. Leading whitespace is skipped; an
/// empty body gives `None`.
fn skip_word(word: &[u8]) -> (Option<&[u8]>, &[u8]) {
    let start = word.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let body = word.get(start..).unwrap_or(b"");
    let end = body
        .iter()
        .take_while(|b| !(b.is_ascii_whitespace() || **b == b')'))
        .count();
    let word = body
        .get(..end)
        .filter(|w| !w.is_empty())
        .or_else(|| body.get(..1));
    (word, body.get(end..).unwrap_or(b""))
}

#[cfg(test)]
mod tests;
