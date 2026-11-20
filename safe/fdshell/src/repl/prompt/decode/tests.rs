#![allow(clippy::unwrap_used)]

use alloc::{vec, vec::Vec};

use super::markers::Markers;
use crate::repl::prompt::segment::{Segment, SegmentKind, ValueEscape};

/// The module's entry point, taken as `&str` for the tests' raw-string
/// inputs (LESSONS: escape-handling tests use raw strings) and the marker
/// policy under test.
fn decode(input: &str, markers: Markers) -> Vec<Segment> {
    super::decode(input.as_bytes(), markers)
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
    assert_eq!(decode(r"a\ab", Markers::Bytes), vec![visible(b"a\x07b")]);
    assert_eq!(decode(r"a\eb", Markers::Bytes), vec![visible(b"a\x1bb")]);
    assert_eq!(decode(r"\a", Markers::Bytes), vec![visible(b"\x07")]);
}

#[test]
fn the_newline_and_the_carriage_return_decode_to_their_bytes() {
    assert_eq!(decode(r"a\nb", Markers::Bytes), vec![visible(b"a\x0ab")]);
    assert_eq!(decode(r"a\rb", Markers::Bytes), vec![visible(b"a\x0db")]);
}

#[test]
fn a_double_backslash_decodes_to_one_backslash() {
    assert_eq!(decode(r"a\\b", Markers::Bytes), vec![visible(b"a\\b")]);
}

#[test]
fn octal_101_decodes_to_capital_a() {
    assert_eq!(decode(r"a\101b", Markers::Bytes), vec![visible(b"aAb")]);
    assert_eq!(decode(r"\101", Markers::Bytes), vec![visible(b"A")]);
}

#[test]
fn octal_needs_exactly_three_digits() {
    // Measured against bash: `\12`, `\7` and `\01` stay literal — unlike
    // printf's one-to-three-digit decoder (LESSONS).
    assert_eq!(decode(r"a\12b", Markers::Bytes), vec![visible(br"a\12b")]);
    assert_eq!(decode(r"a\7b", Markers::Bytes), vec![visible(br"a\7b")]);
    assert_eq!(decode(r"a\01b", Markers::Bytes), vec![visible(br"a\01b")]);
}

#[test]
fn three_octal_digits_are_consumed_and_the_rest_stays_literal() {
    assert_eq!(decode(r"a\1000b", Markers::Bytes), vec![visible(b"a@0b")]);
}

#[test]
fn octal_values_truncate_to_eight_bits() {
    assert_eq!(decode(r"a\777b", Markers::Bytes), vec![visible(b"a\xffb")]);
    // A NUL byte is a byte: bash keeps it mid-string (measured).
    assert_eq!(decode(r"a\400b", Markers::Bytes), vec![visible(b"a\0b")]);
}

#[test]
fn hex_decodes_one_or_two_hex_digits_as_the_fdshell_extension() {
    // Decision D1: bash 5.3.9 measures `\xHH` literal; fdshell decodes one
    // or two hex digits (the README deviation candidate, flagged to #623).
    assert_eq!(decode(r"a\x41b", Markers::Bytes), vec![visible(b"aAb")]);
    assert_eq!(decode(r"a\x4bc", Markers::Bytes), vec![visible(b"aKc")]);
    assert_eq!(decode(r"a\x4zb", Markers::Bytes), vec![visible(b"a\x04zb")]);
}

#[test]
fn a_hex_escape_without_a_hex_digit_stays_literal() {
    assert_eq!(decode(r"a\xZZb", Markers::Bytes), vec![visible(br"a\xZZb")]);
}

#[test]
fn unknown_escapes_and_a_trailing_backslash_stay_literal() {
    assert_eq!(decode(r"a\zb", Markers::Bytes), vec![visible(br"a\zb")]);
    assert_eq!(decode(r"a\Fb", Markers::Bytes), vec![visible(br"a\Fb")]);
    assert_eq!(decode(r"a\", Markers::Bytes), vec![visible(br"a\")]);
}

#[test]
fn a_matched_pair_records_the_region_and_consumes_the_markers() {
    // Measured: the region bytes are drawn and the toggling markers consumed.
    assert_eq!(
        decode(r"a\[b\]c", Markers::Bytes),
        vec![visible(b"a"), non_printing(b"b"), visible(b"c")]
    );
    assert_eq!(
        decode(r"a\[\033[31mB\033[0m\]C", Markers::Bytes),
        vec![
            visible(b"a"),
            non_printing(b"\x1b[31mB\x1b[0m"),
            visible(b"C")
        ],
    );
}

#[test]
fn a_close_marker_that_closes_nothing_is_data() {
    assert_eq!(decode(r"a\]b", Markers::Bytes), vec![visible(b"a\x02b")]);
    assert_eq!(
        decode(r"a\]\]b", Markers::Bytes),
        vec![visible(b"a\x02\x02b")]
    );
}

#[test]
fn an_open_marker_inside_a_region_is_data() {
    // The marker byte lands in the current run — the record keeps the
    // boundary, not just the payload (LESSONS).
    assert_eq!(
        decode(r"a\[\033[31mB\[", Markers::Bytes),
        vec![visible(b"a"), non_printing(b"\x1b[31mB\x01")],
    );
}

#[test]
fn value_escapes_are_recorded_as_placeholders_with_their_text() {
    assert_eq!(
        decode(r"a\u-\v-\$", Markers::Bytes),
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

#[test]
fn the_dropped_policy_draws_nothing_for_a_marker_escape() {
    // The raw drawer (`PS4`, task #621), measured against bash 5.3.9 on
    // 2026-11-20: the non-toggling marker escape is consumed and contributes
    // no bytes, the same escapes the prompt drawer emits as `\001`/`\002`.
    // A matched pair still keeps its region bytes, and an octal `\001` is
    // data — the strip is marker-position-based, never byte-value-based.
    assert_eq!(decode(r"a\]b", Markers::Dropped), vec![visible(b"ab")]);
    assert_eq!(
        decode(r"a\[b\]c", Markers::Dropped),
        vec![visible(b"a"), non_printing(b"b"), visible(b"c")]
    );
    assert_eq!(
        decode(r"a\[\033[31mB\[", Markers::Dropped),
        vec![visible(b"a"), non_printing(b"\x1b[31mB")],
    );
    assert_eq!(
        decode(r"a\001b", Markers::Dropped),
        vec![visible(b"a\x01b")],
        "the same bytes as the `Bytes` row: an octal byte is never a marker"
    );
}
