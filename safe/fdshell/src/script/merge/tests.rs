//! Unit pins of the runner's join of a here-doc-split cond list (task #492).

#![allow(clippy::unwrap_used)]
use super::{Joined, units};
use crate::segment::Segment;
use alloc::vec;
use alloc::vec::Vec;

/// A statement segment, as the scanner would report it: `join` is the mark the
/// scanner leaves when it could not join the run itself (task #492).
fn stmt(cmd: &[u8], bodies: Vec<Vec<u8>>, background: bool, join: bool) -> Segment<'_> {
    Segment::Statement {
        cmd,
        off: 0,
        bodies,
        background,
        midrun_open: false,
        join_next: join,
    }
}

/// The joined statement of the first unit, or `None`.
fn joined(segments: &[Segment<'_>]) -> Option<Joined> {
    let units = units(segments);
    let found = units.first()?.joined.as_ref()?;
    Some(Joined {
        text: found.text.clone(),
        bodies: found.bodies.clone(),
        background: found.background,
    })
}

/// The plan's row 19: the scanner marks the run it could not join, and the
/// runner joins it with the line the operator continues onto — as the same-line
/// spelling, so the parse layer sees the operator as an operator token (the
/// separator is a space; a newline would split the chain back into statements).
#[test]
fn joins_a_run_that_wrote_a_heredoc() {
    let segments = vec![
        stmt(b"echo a <<B &&", vec![b"body\n".to_vec()], false, true),
        stmt(b"echo b", Vec::new(), false, false),
    ];
    let joined = joined(&segments).unwrap();
    assert_eq!(joined.text, b"echo a <<B && echo b".to_vec());
    assert_eq!(joined.bodies, vec![b"body\n".to_vec()]);
    assert!(!joined.background);
    assert_eq!(units(&segments).len(), 1, "the chain folds into its leader");
}

/// The separator keeps the two statements apart: nothing ends on an operator.
#[test]
fn a_word_ending_the_text_joins_nothing() {
    let segments = vec![
        stmt(b"echo a <<B", vec![b"body\n".to_vec()], false, false),
        stmt(b"echo b", Vec::new(), false, false),
    ];
    assert!(joined(&segments).is_none());
    assert_eq!(units(&segments).len(), 2);
}

/// A block segment is its own statement; it is never folded into the chain.
#[test]
fn a_block_follower_stops_the_chain() {
    let segments = vec![
        stmt(b"echo a <<B &&", vec![b"body\n".to_vec()], false, true),
        Segment::Block {
            block_start: 0,
            end_pos: 10,
            closed: true,
            bodies: Vec::new(),
        },
    ];
    assert!(joined(&segments).is_none());
    assert_eq!(units(&segments).len(), 2);
}

/// The chain is transitive — every link lands on the one line the chain stands
/// for, so a chain of two operators is one statement, not two — and the bodies
/// stay in operator order.
#[test]
fn a_chain_concatenates_bodies_in_operator_order() {
    let segments = vec![
        stmt(b"cat <<A &&", vec![b"a\n".to_vec()], false, true),
        stmt(b"cat <<B &&", vec![b"b\n".to_vec()], false, true),
        stmt(b"echo c", Vec::new(), false, false),
    ];
    let joined = joined(&segments).unwrap();
    assert_eq!(
        joined.text,
        b"cat <<A && cat <<B && echo c".to_vec(),
        "the chain is transitive"
    );
    assert_eq!(
        joined.bodies,
        vec![b"a\n".to_vec(), b"b\n".to_vec()],
        "the bodies stay in operator order"
    );
    assert_eq!(units(&segments).len(), 1);
}

/// The plain-`&` flag is the one on the list's last part (task #326).
#[test]
fn the_last_part_keeps_the_background_flag() {
    let segments = vec![
        stmt(b"echo a <<B &&", vec![b"body\n".to_vec()], false, true),
        stmt(b"echo b", Vec::new(), true, false),
    ];
    assert!(joined(&segments).unwrap().background);
}
