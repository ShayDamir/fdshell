#![allow(clippy::unwrap_used)]
use super::region::measure;
use crate::error::parse::ParseError;

/// The depth walk's `<` bound is asked exactly where it carries the rule: a
/// region whose depth never returns at the line's end. `<=` hands the walk the
/// end position itself (a no-op step, then the same answer, or a spin).
#[test]
fn the_walk_bound_answers_at_the_line_end() {
    let err = measure(b"for ((i=0;i<2", 4).err().unwrap();
    assert!(
        matches!(err.current_context(), ParseError::ArithForMissingCloseParen),
        "{err:?}"
    );
    assert!(measure(b"for ((i<2))", 4).unwrap().is_some());
}
