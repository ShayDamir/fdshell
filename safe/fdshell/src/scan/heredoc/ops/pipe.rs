//! The two byte-level mirrors the `<<` scan needs from the tokenizer: "is
//! there a token before this word" and "is the `|` in front of it a pipeline
//! pipe". Both are written on the tokenizer's rules, byte-wise (LESSONS: the
//! byte-level and token-level counts must agree), and both are read by
//! [`crate::scan::heredoc::operator_count`]'s walk.

/// The byte at a consumed position starts (or continues) a token: any byte the
/// tokenizer reads as word content or as a separator token, except a pipeline
/// `|`, which is a stage boundary with no token of its own —
/// `parse::heredoc::is_operator` rejects a `<<` whose previous token is `|`, and
/// a `|`-leading run has no preceding token at all (`| <<EOF`). A `|` absorbed
/// into a `>|` clobber is operator content of its word, decided by
/// [`precedes_pipe`], never here. `;`/`&&` ARE tokens for the tokenizer, so they
/// count here: `cat <<A ; <<B` is two operators at both levels. So is a newline:
/// the tokenizer's separator arm (`parse::token::step`) emits a `;` token for
/// `\n` and `)`, and no token for a space or a tab — those two bytes are the
/// whole non-token list (task #261, `is_token_byte`'s whitespace rows).
pub(super) fn is_token_byte(b: u8) -> bool {
    b != b' ' && b != b'\t' && b != b'|'
}

/// A token byte sits in `line[from..run]`: the byte mirror of `is_operator`'s
/// `i != 0` for an operator whose word begins at `run` (its io-number digits
/// belong to that word, so they never count as the preceding token — a run
/// that opens with only whitespace has none, exactly as the whitespace-leading
/// `<<` rows of [`is_token_byte`] decide).
pub(super) fn token_before(line: &[u8], from: usize, run: usize) -> bool {
    line.get(from..run)
        .unwrap_or_default()
        .iter()
        .any(|&b| is_token_byte(b))
}

/// A `|` with only whitespace between it and `i`: a pipeline position, where
/// `<<` is a command word, not an operator. A `|` absorbed into a `>|` clobber
/// operator is not a pipeline position, and a quoted or escaped `|` is word
/// content, never a pipe: [`crate::parse::redirect::pipeline_pipe`] is the ONE
/// rule for all three, shared with the tokenizer, so the byte and token counts
/// cannot diverge (LESSONS: byte-level and token-level rules must agree).
pub(super) fn precedes_pipe(line: &[u8], i: usize) -> bool {
    let mut k = i;
    while k > 0 {
        k -= 1;
        match line.get(k) {
            Some(&b' ') | Some(b'\t') => continue,
            Some(b'|') => return crate::parse::pipeline_pipe(line, k),
            _ => return false,
        }
    }
    false
}
