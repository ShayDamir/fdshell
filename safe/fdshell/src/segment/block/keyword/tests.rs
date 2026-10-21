#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::ScanState;
use alloc::vec;
use alloc::vec::Vec;

/// Run the keyword-block scan over a whole line, as the segment scanner calls it.
fn scan(line: &[u8]) -> (usize, usize, bool, usize) {
    let (block_start, end_pos, closed, resume, _bodies) = scan_at(line, 0);
    (block_start, end_pos, closed, resume)
}

/// Run the keyword-block scan for the block opening at `start` on a line whose
/// physical line starts at 0, as the segment scanner calls it. `pre_resume` is
/// the line end: the pin asserts on the span and the body regions, not on the
/// floor wiring (pinned by `segment/tests.rs`).
fn scan_at(line: &[u8], start: usize) -> (usize, usize, bool, usize, Vec<Vec<u8>>) {
    let raw = line.get(start..).unwrap_or(b"");
    let part = raw.trim_ascii();
    let (segment, resume, _floor) =
        keyword_block(line, raw, part, start, 0, &ScanState::new(), line.len());
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies,
        } => (block_start, end_pos, closed, resume, bodies),
        Segment::Statement { cmd, .. } => panic!("expected a Block, got {cmd:?}"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}

/// The block scan is seeded *after the keyword*: `kw_len` is measured on the word
/// behind the `time [-p]` prefix run, so a prefixed block is seeded at the same
/// place the unprefixed one is (+ the prefix's length) and its scan sees the
/// keywords written at the start of the condition.
#[test]
fn prefixed_keyword_scan_lands_where_the_unprefixed_one_does() {
    for tail in [
        b"if fi; then :; fi".as_slice(),
        b"if :; then echo y; fi".as_slice(),
        b"for fi in 1; do echo $fi; done".as_slice(),
        b"while :; do :; done".as_slice(),
        b"case x in x) echo X;; esac".as_slice(),
    ] {
        for prefix in [b"".as_slice(), b"time ".as_slice(), b"time -p ".as_slice()] {
            let mut line = prefix.to_vec();
            line.extend_from_slice(tail);
            let (start, end, closed, resume) = scan(&line);
            assert!(closed, "{line:?}");
            assert_eq!(start, 0, "{line:?}");
            // The scan ends at the block's closing keyword, past the prefix.
            let want = scan(tail);
            let offset = time_prefix_len(&line).unwrap_or(0);
            assert_eq!(end, want.1 + offset, "{line:?}");
            assert_eq!(resume, want.3 + offset, "{line:?}");
        }
    }
}

/// A pin: (line, block start, expected span end, expected claimed body bytes).
type Appended = (&'static [u8], usize, usize, Vec<Vec<u8>>);

/// Append mode: the block claims **its own** bodies of its physical line's
/// body stream (the regions of operators `p..p+k`, where `p` is the number of
/// `<<` operators written on that line before the block) and its span stops at
/// the closing keyword's separator, so a trailing statement's text stays
/// outside the block.
#[test]
fn append_mode_claims_the_blocks_own_regions() {
    let cases: [Appended; 5] = [
        // p = 1: the pre-block `cat <<X` owns region 0, the condition takes 1.
        (
            b"cat <<X; if cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB",
            9,
            36,
            vec![b"bodyA\nA\n".to_vec()],
        ),
        // p = 2: two pre-block operators own regions 0 and 1.
        (
            b"cat <<X; cat <<Y; if cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyY\nY\nbodyA\nA\nbodyB\nB",
            18,
            45,
            vec![b"bodyA\nA\n".to_vec()],
        ),
        // p = 0, trailing statement: the regions are contiguous with the text.
        (
            b"if cat <<A; then echo y; fi; cat <<B\nbodyA\nA\nbodyB\nB",
            0,
            27,
            vec![b"bodyA\nA\n".to_vec()],
        ),
        // k = 0 (no operator in the block text): no regions are claimed.
        (
            b"cat <<X; if true; then echo y; fi; cat <<B\nbodyX\nX\nbodyB\nB",
            9,
            33,
            vec![],
        ),
        // A `;`-terminated heredoc in a single-line block body: the region
        // starts just past the closing keyword's separator, so the rebuilt text
        // (`cmd + "\n" + region`) is byte-identical to the extended span.
        (b"if true; then cat <<T; fi\nbodyT\nT", 0, 25, vec![b"bodyT\nT".to_vec()]),
    ];
    for (line, start, end_pos, bodies) in cases {
        let got = scan_at(line, start);
        assert!(got.2, "{line:?} must close");
        assert_eq!(got.0, start, "{line:?}");
        assert_eq!(got.1, end_pos, "{line:?}: span end");
        assert_eq!(got.4, bodies, "{line:?}: claimed bodies");
    }
}

/// Extension mode: bodies interleaved with the block's own lines keep the
/// forward-only extension and carry no body regions — the span covers the whole
/// block text verbatim, exactly as before the region slicing.
#[test]
fn extension_mode_keeps_the_span_and_no_bodies() {
    // A multi-line block whose condition carries a heredoc: the region starts
    // before the closing keyword's separator, so the span is extended to cover
    // the block's own lines.
    let line = b"if cat <<A; then\necho y\nfi\nbodyA\nA";
    let got = scan_at(line, 0);
    assert_eq!(got.1, line.len(), "span end");
    assert_eq!(got.4, Vec::<Vec<u8>>::new(), "no bodies in extension mode");

    // Missing bodies: the region list is empty and the extension falls back to
    // the line's resume, with no regions carried.
    let line = b"while cat <<X; do a && b; X\ndone\necho after";
    let got = scan_at(line, 0);
    assert_eq!(got.1, 32, "span end is the closing keyword's separator");
    assert_eq!(got.4, Vec::<Vec<u8>>::new());
}

/// A pre-block statement on an **earlier** physical line claims nothing in the
/// block's own line's stream: the skip is bounded to the line the block opens
/// on (the comment-deferred flush case).
#[test]
fn skip_is_bounded_to_the_blocks_physical_line() {
    // `line_start` = the start of the block's own line, so the pre-block
    // operator above it is not counted: the condition takes region 0.
    let line = b"cat <<X\nif cat <<A; then echo y; fi; cat <<B\nbodyX\nX\nbodyA\nA\nbodyB\nB";
    let raw = line.get(8..).unwrap_or(b"");
    let part = raw.trim_ascii();
    let (segment, _resume, _floor) =
        keyword_block(line, raw, part, 8, 8, &ScanState::new(), line.len());
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
            bodies,
        } => {
            assert_eq!(block_start, 8);
            assert!(closed);
            // The block's line owns the stream from 8: its bodies start after
            // *that* line's newline, and `<<B`'s body is outside the span.
            assert_eq!(end_pos, 35);
            assert_eq!(bodies, vec![b"bodyX\nX\nbodyA\nA\n".to_vec()]);
        }
        Segment::Statement { cmd, .. } => panic!("expected a Block, got {cmd:?}"),
        Segment::AsyncInvalid => panic!("unexpected async marker"),
    }
}
