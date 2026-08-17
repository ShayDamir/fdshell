#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;

#[test]
fn keyword_delta_closer_with_extra_boundary() {
    assert_eq!(keyword_delta(b"fi|"), Some(-1));
    assert_eq!(keyword_delta(b"done&"), Some(-1));
    assert_eq!(keyword_delta(b"esac|"), Some(-1));
}

#[test]
fn keyword_delta_opener_with_semi_boundary() {
    assert_eq!(keyword_delta(b"if;"), Some(1));
    assert_eq!(keyword_delta(b"for;"), Some(1));
}
