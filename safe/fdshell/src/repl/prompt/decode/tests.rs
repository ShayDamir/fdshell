#![allow(clippy::unwrap_used)]

use alloc::{vec, vec::Vec};

use crate::repl::prompt::segment::{Segment, SegmentKind, ValueEscape};

/// The module's entry point, taken as `&str` for the tests' raw-string
/// inputs (LESSONS: escape-handling tests use raw strings).
fn decode(input: &str) -> Vec<Segment> {
    super::decode(input.as_bytes())
}

/// One visible run.
fn visible(bytes: &[u8]) -> Segment {
    Segment {
        kind: SegmentKind::Visible,
        bytes: bytes.to_vec(),
    }
}

/// One non-printing run between matched markers.
fn non_printing(bytes: &[u8]) -> Segment {
    Segment {
        kind: SegmentKind::NonPrinting,
        bytes: bytes.to_vec(),
    }
}

/// A value placeholder carrying its raw escape text.
fn placeholder(esc: ValueEscape, text: &[u8]) -> Segment {
    Segment {
        kind: SegmentKind::Value(esc),
        bytes: text.to_vec(),
    }
}

#[test]
fn a_bell_and_an_escape_land_in_the_current_run() {
    assert_eq!(decode(r"a\ab"), vec![visible(b"a\x07b")]);
    assert_eq!(decode(r"a\eb"), vec![visible(b"a\x1bb")]);
    assert_eq!(decode(r"\a"), vec![visible(b"\x07")]);
}

#[test]
fn the_newline_and_the_carriage_return_decode_to_their_bytes() {
    assert_eq!(decode(r"a\nb"), vec![visible(b"a\x0ab")]);
    assert_eq!(decode(r"a\rb"), vec![visible(b"a\x0db")]);
}

#[test]
fn a_double_backslash_decodes_to_one_backslash() {
    assert_eq!(decode(r"a\\b"), vec![visible(b"a\\b")]);
}

#[test]
fn octal_101_decodes_to_capital_a() {
    assert_eq!(decode(r"a\101b"), vec![visible(b"aAb")]);
    assert_eq!(decode(r"\101"), vec![visible(b"A")]);
}

#[test]
fn octal_needs_exactly_three_digits() {
    // Measured against bash: `\12`, `\7` and `\01` stay literal — unlike
    // printf's one-to-three-digit decoder (LESSONS).
    assert_eq!(decode(r"a\12b"), vec![visible(br"a\12b")]);
    assert_eq!(decode(r"a\7b"), vec![visible(br"a\7b")]);
    assert_eq!(decode(r"a\01b"), vec![visible(br"a\01b")]);
}

#[test]
fn three_octal_digits_are_consumed_and_the_rest_stays_literal() {
    assert_eq!(decode(r"a\1000b"), vec![visible(b"a@0b")]);
}

#[test]
fn octal_values_truncate_to_eight_bits() {
    assert_eq!(decode(r"a\777b"), vec![visible(b"a\xffb")]);
    // A NUL byte is a byte: bash keeps it mid-string (measured).
    assert_eq!(decode(r"a\400b"), vec![visible(b"a\0b")]);
}

#[test]
fn hex_decodes_one_or_two_hex_digits_as_the_fdshell_extension() {
    // Decision D1: bash 5.3.9 measures `\xHH` literal; fdshell decodes one
    // or two hex digits (the README deviation candidate, flagged to #623).
    assert_eq!(decode(r"a\x41b"), vec![visible(b"aAb")]);
    assert_eq!(decode(r"a\x4bc"), vec![visible(b"aKc")]);
    assert_eq!(decode(r"a\x4zb"), vec![visible(b"a\x04zb")]);
}

#[test]
fn a_hex_escape_without_a_hex_digit_stays_literal() {
    assert_eq!(decode(r"a\xZZb"), vec![visible(br"a\xZZb")]);
}

#[test]
fn unknown_escapes_and_a_trailing_backslash_stay_literal() {
    assert_eq!(decode(r"a\zb"), vec![visible(br"a\zb")]);
    assert_eq!(decode(r"a\Fb"), vec![visible(br"a\Fb")]);
    assert_eq!(decode(r"a\"), vec![visible(br"a\")]);
}

#[test]
fn a_matched_pair_records_the_region_and_consumes_the_markers() {
    // Measured: the region bytes are drawn and the toggling markers consumed.
    assert_eq!(
        decode(r"a\[b\]c"),
        vec![visible(b"a"), non_printing(b"b"), visible(b"c")]
    );
    assert_eq!(
        decode(r"a\[\033[31mB\033[0m\]C"),
        vec![
            visible(b"a"),
            non_printing(b"\x1b[31mB\x1b[0m"),
            visible(b"C")
        ],
    );
}

#[test]
fn a_close_marker_that_closes_nothing_is_data() {
    assert_eq!(decode(r"a\]b"), vec![visible(b"a\x02b")]);
    assert_eq!(decode(r"a\]\]b"), vec![visible(b"a\x02\x02b")]);
}

#[test]
fn an_open_marker_inside_a_region_is_data() {
    // The marker byte lands in the current run — the record keeps the
    // boundary, not just the payload (LESSONS).
    assert_eq!(
        decode(r"a\[\033[31mB\["),
        vec![visible(b"a"), non_printing(b"\x1b[31mB\x01")],
    );
}

#[test]
fn value_escapes_are_recorded_as_placeholders_with_their_text() {
    assert_eq!(
        decode(r"a\u-\v-\$"),
        vec![
            visible(b"a"),
            placeholder(ValueEscape::Username, br"\u"),
            visible(b"-"),
            placeholder(ValueEscape::Version, br"\v"),
            visible(b"-"),
            placeholder(ValueEscape::Dollars, br"\$"),
        ],
    );
}
