/// Check that `word` ends at a token boundary at position `len`.
fn boundary(word: &[u8], len: usize, extra: &[u8]) -> bool {
    word.get(len)
        .is_none_or(|&b| b.is_ascii_whitespace() || b == b';' || extra.contains(&b))
}

/// Return `Some(1)` for keywords that open a block, `Some(-1)` for closers.
///
/// Recognized: `case`, `esac`, `if`, `fi`, `for`, `while`, `until`, `done`.
/// Each check requires a word boundary after the keyword to avoid
/// matching prefixes like `ifconfig` or `donec`.
pub(super) fn keyword_delta(word: &[u8]) -> Option<i32> {
    if word.starts_with(b"case") && boundary(word, 4, b"") {
        return Some(1);
    }
    if word.starts_with(b"esac") && boundary(word, 4, b"&|") {
        return Some(-1);
    }
    if word.starts_with(b"if") && boundary(word, 2, b"") {
        return Some(1);
    }
    if word.starts_with(b"fi") && boundary(word, 2, b"&|") {
        return Some(-1);
    }
    if word.starts_with(b"for") && boundary(word, 3, b"") {
        return Some(1);
    }
    if (word.starts_with(b"while") || word.starts_with(b"until")) && boundary(word, 5, b"") {
        return Some(1);
    }
    if word.starts_with(b"done") && boundary(word, 4, b"&|") {
        return Some(-1);
    }
    None
}
