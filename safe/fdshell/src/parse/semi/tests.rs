#![allow(clippy::unwrap_used)]
use super::try_join_masked;
use crate::parse::token::tokenize;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// `try_join_masked` of every token of `line`: the joined text, its mask and its
/// zero-byte span list. Task #321: the spans shift by the join offset, and a
/// joined list nothing walks is a free mutant — so the lists are pinned here.
fn join(line: &[u8]) -> (ShortCStr, Vec<bool>, Vec<usize>) {
    try_join_masked(&tokenize(line).unwrap())
}

#[test]
fn joined_spans_shift_by_the_join_offset() {
    // `~/a b""`: the pair contributes no bytes and lands at the join's last
    // index. Its index 1 is shifted by the offset accumulated before the token
    // (3 = `~/a`), so the joined list is `[4]` — a shift to `-` gives 2 and a
    // shift to `*` gives 3, and the tilde gate reads a different prefix.
    let (text, mask, spans) = join(b"~/a b\"\"");
    assert_eq!(text, ShortCStr::from(c"~/a b"));
    assert_eq!(mask, vec![false; 5], "mask parallel to the text");
    assert_eq!(spans, vec![4]);
    // A leading pair keeps its index 0 (`""~/x` is the tilde-prefix shape the
    // gate reads at 0) and the second token's pair is shifted: `[0, 4]`.
    let (text, mask, spans) = join(b"\"\"~/x b\"\"");
    assert_eq!(text, ShortCStr::from(c"~/x b"));
    assert_eq!(mask, vec![false; 5]);
    assert_eq!(spans, vec![0, 4]);
}

/// The `case` word is a join (`case_block` takes `try_join_masked` of the
/// tokens between `case` and `in`), so its zero-byte spans travel through the
/// same shift. One token means offset 0: a `-` shift underflows and a `*`
/// shift zeroes the index, i.e. the gate loses the pair.
#[test]
fn case_word_join_keeps_the_zero_byte_span() {
    let tokens = tokenize(b"case ~\"\" in x) ;; esac").unwrap();
    let in_idx = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(b"in"))
        .unwrap();
    let (word, word_mask, word_span) = try_join_masked(tokens.get(1..in_idx).unwrap());
    assert_eq!(word, ShortCStr::from(c"~"));
    assert_eq!(word_mask, vec![false]);
    assert_eq!(word_span, vec![1]);
    // Two word tokens with a pair in each: the offsets are 0 and 1, and the
    // joined list keeps each pair's own shifted index.
    let tokens = tokenize(b"case ~\"\" x\"\" in x) ;; esac").unwrap();
    let in_idx = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(b"in"))
        .unwrap();
    let (word, word_mask, word_span) = try_join_masked(tokens.get(1..in_idx).unwrap());
    assert_eq!(word, ShortCStr::from(c"~ x"));
    assert_eq!(word_mask, vec![false; 3], "mask parallel to the text");
    assert_eq!(word_span, vec![1, 2]);
}
