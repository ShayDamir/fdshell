#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{operator_delim, untab, untab_body, Operator};

#[test]
fn operator_delim_attached_word_is_plain() {
    let line = b"<<WORD";
    assert_eq!(operator_delim(line, 0, 6), (false, b"WORD" as &[u8]));
}

#[test]
fn operator_delim_dash_marker_takes_the_word() {
    let line = b"<<-WORD";
    assert_eq!(operator_delim(line, 0, 7), (true, b"WORD" as &[u8]));
}

#[test]
fn operator_delim_dash_only_is_bare_marker() {
    let line = b"<<-";
    assert_eq!(operator_delim(line, 0, 3), (true, b"" as &[u8]));
}

#[test]
fn operator_delim_consumes_exactly_one_dash() {
    // `<<---`: one `-` is the marker, `--` is the delimiter.
    let line = b"<<---";
    assert_eq!(operator_delim(line, 0, 5), (true, b"--" as &[u8]));
}

#[test]
fn operator_delim_keeps_quotes_for_the_operator() {
    // The quotes are stripped by `Operator::new`, not by the byte rule.
    let line = b"<<\"Q\"";
    assert_eq!(operator_delim(line, 0, 5), (false, b"\"Q\"" as &[u8]));
}

#[test]
fn untab_strips_leading_tabs_only() {
    assert_eq!(untab(b"\tone"), b"one" as &[u8]);
    assert_eq!(untab(b"\t\ta\tb"), b"a\tb" as &[u8]);
    assert_eq!(untab(b"  sp"), b"  sp" as &[u8]);
    assert_eq!(untab(b"plain"), b"plain" as &[u8]);
    assert_eq!(untab(b""), b"" as &[u8]);
}

#[test]
fn untab_body_strips_leading_tabs_of_every_line() {
    assert_eq!(
        untab_body(b"\tone\n\t\ttwo\nthree\n").as_slice(),
        b"one\ntwo\nthree\n" as &[u8]
    );
    assert_eq!(untab_body(b"").as_slice(), b"" as &[u8]);
    // A tabs-only line becomes empty, keeping its terminating newline.
    assert_eq!(untab_body(b"\t\t\n").as_slice(), b"\n" as &[u8]);
    assert_eq!(
        untab_body(b"body\n\t\t\n").as_slice(),
        b"body\n\n" as &[u8]
    );
}

#[test]
fn operator_matches_respects_strip() {
    let plain = Operator::new(b"EOF", false);
    assert!(plain.matches(b"EOF"));
    assert!(!plain.matches(b"\tEOF"), "the plain form never strips tabs");
    let dash = Operator::new(b"EOF", true);
    assert!(dash.matches(b"EOF"));
    assert!(dash.matches(b"\tEOF"));
    assert!(dash.matches(b"\t\tEOF"));
    assert!(!dash.matches(b" xEOF"));
    assert!(!dash.matches(b"EOFx"));
}

#[test]
fn operator_body_bytes_respects_strip() {
    let plain = Operator::new(b"EOF", false);
    assert_eq!(plain.body_bytes(b"\tone\n").as_slice(), b"\tone\n" as &[u8]);
    let dash = Operator::new(b"EOF", true);
    assert_eq!(
        dash.body_bytes(b"\tone\n\t\ttwo\n").as_slice(),
        b"one\ntwo\n" as &[u8]
    );
}

#[test]
fn operator_new_strips_quotes_and_carries_flags() {
    let q = Operator::new(b"\"Q\"", true);
    assert_eq!(q.delim, b"Q" as &[u8]);
    assert!(q.quoted);
    assert!(q.strip);
    let p = Operator::new(b"EOF", false);
    assert_eq!(p.delim, b"EOF" as &[u8]);
    assert!(!p.quoted);
    assert!(!p.strip);
}
