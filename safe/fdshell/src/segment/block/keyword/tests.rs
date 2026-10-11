#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::scan::ScanState;

/// Run the keyword-block scan over a whole line, as the segment scanner calls it.
fn scan(line: &[u8]) -> (usize, usize, bool, usize) {
    let part = line.trim_ascii();
    let (segment, resume, _floor) =
        keyword_block(line, line, part, 0, &ScanState::new(), line.len());
    match segment {
        Segment::Block {
            block_start,
            end_pos,
            closed,
        } => (block_start, end_pos, closed, resume),
        Segment::Statement { cmd, .. } => panic!("expected a Block, got {cmd:?}"),
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
