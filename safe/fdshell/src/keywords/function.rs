//! The function-definition opener rule, shared by the byte-level segment
//! scanner (`brace::scan_function_block`) and the token-level parse dispatch
//! (`parse::function_block`), so the two layers accept the same shapes.

use super::quoted_word;

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
///
/// `start` is the index in `line` of `word`'s **first byte**. Every caller
/// hands over the byte its own text starts at, never the run's untrimmed start:
/// the gate walks `line` from `word`'s leading whitespace, exactly as the
/// sibling keyword scan seeds itself with `start + leading_ws + kw_len`.
///
/// The quote gate (task #247, [`quoted_word`]): a definer word written with
/// quote bytes is word data — bash rejects `'function' f() { … }`,
/// `function 'f'() { … }` and `f'oo'() { … }` (`'f'`: not a valid identifier`)
/// and fdshell must not claim the block at the byte level either, or its
/// quote-removal rewrites the definer into a shape the parse layer accepts.
/// `quoted_word` breaks its scan at the POSIX `(`, so the gate reads the name's
/// own bytes: the head minus its `(` in the POSIX form, the name word's own end
/// in the keyword form.
pub(crate) fn function_def_name<'a>(word: &'a [u8], line: &[u8], start: usize) -> Option<&'a [u8]> {
    let (head, second) = def_words(word);
    let head_len = head?.len();
    let head_end = start.checked_add(leading_ws(word))?.checked_add(head_len)?;
    // The walk from the head to the name word, measured on `line`.
    let name_start = head_end.checked_add(gap(line.get(head_end..)?))?;
    let posix_end = head_end.checked_sub(1)?;
    if head == Some(b"function") {
        let name_word = name(second.filter(|w| name(w).is_some())?)?;
        let name_end = name_start.checked_add(name_word.len())?;
        return (!quoted_word(line, name_end)).then_some(name_word);
    }
    head.filter(|w| w.ends_with(b"("))
        .and_then(name)
        .filter(|_| !quoted_word(line, posix_end))
}

/// The byte length of the definer words at the start of `word`: the definer
/// text the parse layer consumes (`f(` + its `)` in the POSIX form,
/// `function` + the name word in the keyword form). Leading whitespace is
/// skipped, the text after the definer words is not counted. `None` = not a
/// definer. `segment/group/prefix.rs::definer_at_run` measures the filler
/// behind the returned length, so the run rule and this definer rule are one
/// implementation (task #235).
pub(crate) fn definer_len(word: &[u8], line: &[u8], start: usize) -> Option<usize> {
    function_def_name(word, line, start)?;
    let (head, rest) = skip_word(word);
    if head.is_some_and(|w| w == b"function") {
        return Some(word.len() - skip_word(rest).1.len());
    }
    // POSIX: the `)` closing the name word's `(` is part of the definer text.
    Some(word.len() - rest.len() + usize::from(rest.first() == Some(&b')')))
}

/// The definer word with its trailing POSIX `(` stripped.
fn name(word: &[u8]) -> Option<&[u8]> {
    word.get(..name_len(word)?)
}

/// The whitespace run at the start of `word`: `word`'s first byte sits at
/// `start + leading_ws(word)` in the line the caller passed — the seed term
/// [`function_def_name`] and `keyword_block`'s scan both need (LESSONS).
pub(crate) fn leading_ws(word: &[u8]) -> usize {
    word.iter().take_while(|b| b.is_ascii_whitespace()).count()
}

/// The gap between two definer words: the whitespace `skip_word` leaves at the
/// first word's end, plus the `)` it stops at (the keyword form's `name)` split).
fn gap(rest: &[u8]) -> usize {
    leading_ws(rest) + usize::from(rest.first() == Some(&b')'))
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
    let body = word.get(leading_ws(word)..).unwrap_or(b"");
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
