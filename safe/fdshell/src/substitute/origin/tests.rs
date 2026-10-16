#![allow(clippy::unwrap_used)]

use super::Origin;

#[test]
fn record_pads_the_gap_before_the_expansion() {
    // Text `x<ab>`: the literal `x` is padded false, the span marked true.
    let mut origin = Origin::new();
    origin.record(1, 3);
    origin.close(4);
    assert!(!origin.splittable(0));
    assert!(origin.splittable(1));
    assert!(origin.splittable(2));
    assert!(!origin.splittable(3));
}

#[test]
fn record_of_an_empty_span_records_nothing() {
    // An arm that produced no bytes adds no splittable byte: `is_empty`
    // stays the "an arm ran whose bytes may split" signal of task #133.
    let mut origin = Origin::new();
    origin.record(0, 0);
    assert!(origin.is_empty());
    origin.record(2, 2);
    assert!(origin.is_empty());
}

#[test]
fn close_pads_the_literal_tail() {
    let mut origin = Origin::new();
    origin.record(0, 2);
    origin.close(5);
    assert!(origin.splittable(1));
    assert!(!origin.splittable(2));
    assert!(!origin.splittable(4));
}

#[test]
fn all_marks_every_byte_splittable() {
    let origin = Origin::all(3);
    assert!(!origin.is_empty());
    for i in 0..3 {
        assert!(origin.splittable(i));
    }
    assert!(origin.boundaries_at(0).is_empty());
}

#[test]
fn is_empty_means_no_splittable_byte() {
    assert!(Origin::new().is_empty());
    let mut origin = Origin::new();
    origin.boundary(0, true);
    assert!(origin.is_empty(), "a boundary alone delimits nothing");
    origin.record(2, 4);
    assert!(!origin.is_empty());
}

#[test]
fn boundaries_at_finds_the_index_and_keeps_the_quote_bit() {
    let mut origin = Origin::new();
    origin.record(0, 2);
    origin.boundary(2, true);
    origin.boundary(2, false);
    origin.record(2, 3);
    origin.close(3);
    assert!(origin.boundaries_at(1).is_empty());
    let at2 = origin.boundaries_at(2);
    assert_eq!(at2.len(), 2, "adjacent positionals share their start index");
    assert_eq!(at2.first(), Some(&(2, true)));
    assert_eq!(at2.get(1), Some(&(2, false)));
    assert!(origin.boundaries_at(3).is_empty());
}

#[test]
fn ends_quoted_only_for_the_boundary_at_the_word_end() {
    let mut origin = Origin::new();
    origin.boundary(1, true);
    origin.record(1, 2);
    origin.close(3);
    assert!(!origin.ends_quoted(3), "the boundary is not at the end");
    let mut origin = Origin::new();
    origin.record(0, 1);
    origin.boundary(1, true);
    assert!(origin.ends_quoted(1));
    let mut origin = Origin::new();
    origin.record(0, 1);
    origin.boundary(1, false);
    assert!(!origin.ends_quoted(1), "an unquoted empty tail vanishes");
}
