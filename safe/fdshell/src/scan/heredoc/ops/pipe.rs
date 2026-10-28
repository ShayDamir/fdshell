//! The byte-level mirror of the tokenizer's pipeline-`|` rule, read by the
//! `<<` site gate ([`crate::scan::heredoc::operator_count`]'s walk): the one
//! byte question the gate cannot answer with a word break alone.

/// A `|` with only whitespace between it and `i`: the tokenizer's pipeline pipe,
/// a stage boundary that ends the previous stage's word and starts a fresh one,
/// so the `<<` written right behind it begins a word (`| <<EOF`, `a|<<EOF`).
/// A `|` absorbed into a `>|` clobber operator is operator content of its word,
/// not a break, and a quoted or escaped `|` is word content, never a pipe:
/// [`crate::parse::pipeline_pipe`] is the ONE rule for all three, shared with
/// the tokenizer, so the byte and token counts cannot diverge (LESSONS:
/// byte-level and token-level rules must agree). The token-level half of the
/// same shape is `parse::heredoc::operator_indices`, which starts its scan at
/// index 0 and so reads the first word of every stage (task #345).
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
