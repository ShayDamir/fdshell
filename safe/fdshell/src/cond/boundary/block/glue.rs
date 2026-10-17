//! The pair-glue strip of the byte-level block scan (task #236's S2): where
//! a word starting at `i` sits inside a glued `&&` / `||` cond pair.

/// The number of pair bytes a word at `i` starts with: `2` at a glued pair's
/// head (`||if`), `1` at its tail byte (`&if`, `|if`), `0` outside a pair —
/// the *pipeline* `|` is a single-byte run, not a pair.
pub(super) fn strip_at(line: &[u8], i: usize) -> usize {
    let Some(c) = line.get(i).copied().filter(|&c| c == b'|' || c == b'&') else {
        return 0;
    };
    if line.get(i.wrapping_sub(1)) == Some(&c) {
        1
    } else if line.get(i + 1) == Some(&c) {
        2
    } else {
        0
    }
}
