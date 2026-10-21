//! Unit tests for the quote-mask bit, its reads and the region cap.

use super::{
    MaskBit, MaskBit::Quoted, MaskBit::QuotedStart, MaskBit::Unquoted, quoted_at, region_len,
};
use alloc::{vec, vec::Vec};

/// The mask of a word built from `parts`, `(quoted, byte count)` pairs:
/// `runs(&[(true, 2), (false, 1)])` is the mask of the text of `"ab"c`. Each
/// quoted part is one quote region, so it opens with a `QuotedStart`.
pub(crate) fn runs(parts: &[(bool, usize)]) -> Vec<MaskBit> {
    let mut out = Vec::new();
    for &(quoted, n) in parts {
        for i in 0..n {
            let bit = if !quoted {
                Unquoted
            } else if i == 0 {
                QuotedStart
            } else {
                Quoted
            };
            out.push(bit);
        }
    }
    out
}

#[test]
fn quoted_is_the_bool_reading_of_the_bit() {
    assert!(!Unquoted.quoted());
    assert!(Quoted.quoted());
    assert!(QuotedStart.quoted());
}

#[test]
fn from_quoted_never_claims_a_region_start() {
    assert_eq!(MaskBit::from_quoted(true), Quoted);
    assert_eq!(MaskBit::from_quoted(false), Unquoted);
}

#[test]
fn runs_open_every_quoted_part_with_a_region_start() {
    assert_eq!(
        runs(&[(true, 2), (false, 1), (true, 2)]),
        vec![QuotedStart, Quoted, Unquoted, QuotedStart, Quoted]
    );
    // An empty part contributes no byte, so it opens no region.
    assert_eq!(runs(&[(true, 0), (false, 1)]), vec![Unquoted]);
}

#[test]
fn quoted_at_pads_past_the_end_of_the_mask() {
    let mask = [QuotedStart, Quoted, Unquoted];
    assert!(quoted_at(&mask, 0));
    assert!(quoted_at(&mask, 1));
    assert!(!quoted_at(&mask, 2));
    assert!(!quoted_at(&mask, 3), "a missing bit is unquoted");
    assert!(!quoted_at(&[], 0));
}

#[test]
fn region_len_measures_the_trigger_byte_region() {
    // A quoted trigger runs over the `Quoted` bytes that continue its region.
    assert_eq!(region_len(&[QuotedStart, Quoted, Unquoted], 0), 2);
    assert_eq!(region_len(&[QuotedStart, Quoted, Unquoted], 1), 1);
    assert_eq!(region_len(&[QuotedStart, Quoted, Unquoted], 2), 1);
    // An unquoted trigger runs over the unquoted bytes only.
    assert_eq!(region_len(&[Unquoted, Unquoted, QuotedStart], 0), 2);
    // A region start is the trigger with the longest run in a quoted word.
    assert_eq!(region_len(&runs(&[(true, 3)]), 0), 3);
    assert_eq!(region_len(&runs(&[(true, 1)]), 0), 1);
}

#[test]
fn region_len_stops_at_the_next_region_start() {
    // The text of `"$v""b"` is `$vb` over two regions, and the second region's
    // `QuotedStart` ends the name — bash expands it as `v` plus `b`.
    let mask = runs(&[(true, 2), (true, 1)]);
    assert_eq!(mask, vec![QuotedStart, Quoted, QuotedStart]);
    assert_eq!(
        region_len(&mask, 0),
        2,
        "the name gets the one byte left in the region"
    );
    assert_eq!(region_len(&mask, 2), 1, "the second region is its own");
    // Symmetrically, a region start ends an unquoted name: `a"$v"` keeps `a`
    // out of the name, and `$v` inside its own region.
    let mask = runs(&[(false, 2), (true, 2)]);
    assert_eq!(region_len(&mask, 0), 2);
    assert_eq!(
        region_len(&mask, 1),
        1,
        "the region start ends the unquoted run"
    );
}

#[test]
fn region_len_pads_a_missing_bit_as_unbounded() {
    assert_eq!(region_len(&[], 0), usize::MAX);
    assert_eq!(region_len(&[QuotedStart, Quoted], 2), usize::MAX);
}
