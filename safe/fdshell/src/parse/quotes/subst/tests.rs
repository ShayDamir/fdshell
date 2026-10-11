#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::token::State;
use alloc::vec;
use alloc::vec::Vec;

type Bytes = core::iter::Peekable<core::iter::Copied<core::slice::Iter<'static, u8>>>;

/// The state and stream exactly as the token loop leaves them when it has just
/// consumed the byte at `open` (the region's first byte) inside double quotes.
fn at(line: &'static [u8], open: usize) -> (State, Bytes) {
    let mut st = State::new();
    let mut bytes: Bytes = line.iter().copied().peekable();
    for _ in 0..=open {
        bytes.next();
    }
    st.pos = open + 1;
    st.in_quotes = true;
    st.quote_start = Some(open);
    st.word_started = true;
    (st, bytes)
}

/// Every region reader pushes one mask bit per text byte and advances `pos` to
/// the exclusive end of the raw bytes it consumed (LESSONS: `pos` counts raw
/// bytes, `cur`/`mask` count token bytes).
#[test]
fn every_region_reader_keeps_pos_and_mask_aligned() {
    // x"$(" at 2, the region ends just past the `)` at 16.
    let (mut st, mut bytes) = at(b"x\"$(echo \"a b\")\"", 2);
    dollar_paren(&mut st, &mut bytes, b"x\"$(echo \"a b\")\"").unwrap();
    assert_aligned(&st, b"$(echo \"a b\")", 15);

    // `${ … }` with inner quotes: 4 quote bytes are dropped from the text.
    let (mut st, mut bytes) = at(b"x\"${u:-\"z w\"}\"", 2);
    brace(&mut st, &mut bytes, b"x\"${u:-\"z w\"}\"").unwrap();
    assert_aligned(&st, b"${u:-z w}", 13);

    // Backtick region: the same, with 2 quote bytes dropped.
    let (mut st, mut bytes) = at(b"x\"`printf \"q r\"`\"", 2);
    backtick(&mut st, &mut bytes, b"x\"`printf \"q r\"`\"").unwrap();
    assert_aligned(&st, b"`printf q r`", 16);
}

/// An unterminated `${ … }` stops before the unpaired quote and advances `pos`
/// only over the bytes it really consumed, so the loop re-reads the quote and
/// the outer word closes.
#[test]
fn unterminated_brace_region_stops_before_the_unpaired_quote() {
    let line = b"x\"${v#a\"";
    let (mut st, mut bytes) = at(line, 2);
    brace(&mut st, &mut bytes, line).unwrap();
    assert_aligned(&st, b"${v#a", 7);
    assert_eq!(
        bytes.next(),
        Some(b'"'),
        "the unpaired quote is left to the token loop"
    );
}

// An unterminated region with no open inner quote consumes the rest of the
// line, so the enclosing quote stays open and `State::finish` reports it.
#[test]
fn unterminated_region_without_an_inner_quote_consumes_the_tail() {
    let line = b"x\"${u:-y";
    let (mut st, mut bytes) = at(line, 2);
    brace(&mut st, &mut bytes, line).unwrap();
    assert_aligned(&st, b"${u:-y", 8);
    assert_eq!(bytes.next(), None);
    let line = b"x\"`a";
    let (mut st, mut bytes) = at(line, 2);
    backtick(&mut st, &mut bytes, line).unwrap();
    assert_aligned(&st, b"`a", 4);
}

// A region in a word that already accumulated bytes keeps the mask parallel to
// the whole word (the mask is what protects the expansion from IFS splitting).
#[test]
fn a_region_after_word_bytes_keeps_the_mask_parallel() {
    let line = b"x\"$(echo \"a b\")\"";
    let (mut st, mut bytes) = at(line, 2);
    st.cur.push(c"x");
    st.mask.push(false);
    st.start = 0;
    dollar_paren(&mut st, &mut bytes, line).unwrap();
    assert_eq!(st.cur.as_bytes().unwrap(), b"x$(echo \"a b\")");
    let mut mask = vec![false];
    mask.extend(vec![true; 13]);
    assert_eq!(st.mask, mask, "the mask is parallel to the whole word");
}

// The mask bits are `true` for every region byte: the word's expansion must
// stay one word (`fully_quoted` reads exactly these bits).
#[test]
fn region_bytes_are_masked_as_quoted() {
    let line = b"\"${u:-\"z w\"}\"";
    let (mut st, mut bytes) = at(line, 1);
    brace(&mut st, &mut bytes, line).unwrap();
    let mask: Vec<bool> = st.mask.clone();
    assert!(mask.iter().all(|&q| q), "all {} bits", mask.len());
    assert_eq!(mask.len(), 9);
}

fn assert_aligned(st: &State, text: &[u8], pos: usize) {
    assert_eq!(st.cur.as_bytes().unwrap(), text);
    assert_eq!(
        st.mask.len(),
        text.len(),
        "one mask bit per pushed byte: {:?}",
        st.mask
    );
    assert_eq!(st.pos, pos, "pos is the exclusive end of the raw bytes");
}
