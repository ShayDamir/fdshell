//! Unit pins of the runner's join of a here-doc-split cond list (task #492).

#![allow(clippy::unwrap_used)]
use super::{Joined, units};
use crate::segment::Segment;
use alloc::vec;
use alloc::vec::Vec;

/// A statement segment, as the scanner would report it: `join` is the mark the
/// scanner leaves when it could not join the run itself (task #492), and `joins`
/// the offsets **into `cmd`** of the separator newlines it stepped over as the
/// operator's terminator while the run stayed open (the scan-joined run's own
/// text; the chain writes each as a space).
fn stmt(
    cmd: &[u8],
    bodies: Vec<Vec<u8>>,
    background: bool,
    join: bool,
    joins: Vec<usize>,
) -> Segment<'_> {
    Segment::Statement {
        cmd,
        off: 0,
        bodies,
        background,
        midrun_open: false,
        join_next: join,
        joins,
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
        stmt(
            b"echo a <<B &&",
            vec![b"body\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(b"echo b", Vec::new(), false, false, Vec::new()),
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
        stmt(
            b"echo a <<B",
            vec![b"body\n".to_vec()],
            false,
            false,
            Vec::new(),
        ),
        stmt(b"echo b", Vec::new(), false, false, Vec::new()),
    ];
    assert!(joined(&segments).is_none());
    assert_eq!(units(&segments).len(), 2);
}

/// A block segment is its own statement; it is never folded into the chain.
#[test]
fn a_block_follower_stops_the_chain() {
    let segments = vec![
        stmt(
            b"echo a <<B &&",
            vec![b"body\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
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
        stmt(
            b"cat <<A &&",
            vec![b"a\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(
            b"cat <<B &&",
            vec![b"b\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(b"echo c", Vec::new(), false, false, Vec::new()),
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
        stmt(
            b"echo a <<B &&",
            vec![b"body\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(b"echo b", Vec::new(), true, false, Vec::new()),
    ];
    assert!(joined(&segments).unwrap().background);
}

/// A link that is itself a **scan-joined** run carries the separator newlines of the
/// physical lines it spans inside its own text, and the scan left their offsets on the
/// segment (`segment/line.rs::run_joins`). The chain writes every one of them as a
/// space, so the whole chain — not only the merge's own separators — is the one line
/// the parse layer reads it as (task #492, LESSONS: "A `;` token is a newline in
/// disguise"). Left as newlines, the operator at the end of the run's second line
/// stops being an operator token and the chain splits back into statements.
#[test]
fn a_link_the_scan_joined_writes_on_one_line() {
    // `echo c1 &&` = 0..9, its newline at 10; `echo c2 &&` = 11..20, its newline at
    // 21; `echo last` = 22..30.
    let run = b"echo c1 &&\necho c2 &&\necho last";
    let segments = vec![
        stmt(
            b"cat <<B &&",
            vec![b"body\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(run, Vec::new(), false, false, vec![10, 21]),
    ];
    let joined = joined(&segments).unwrap();
    assert_eq!(
        joined.text,
        b"cat <<B && echo c1 && echo c2 && echo last".to_vec(),
        "the run's own separators are written as spaces too"
    );
    assert_eq!(joined.bodies, vec![b"body\n".to_vec()]);
}

/// A newline that is **not** a separator stays a newline: it is data inside a quoted
/// word and the link carries no offset for it (task #492). A chain that normalized it
/// would print the quoted word with a space.
#[test]
fn a_links_data_newline_stays_a_newline() {
    let segments = vec![
        stmt(
            b"cat <<B &&",
            vec![b"body\n".to_vec()],
            false,
            true,
            Vec::new(),
        ),
        stmt(b"echo \"x\ny\"", Vec::new(), false, false, Vec::new()),
    ];
    assert_eq!(
        joined(&segments).unwrap().text,
        b"cat <<B && echo \"x\ny\"".to_vec()
    );
}
