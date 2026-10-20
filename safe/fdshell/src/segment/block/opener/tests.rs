#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::segment::Segment;
use alloc::vec::Vec;

/// Score the run `raw` at its own start (the run is the whole line here).
fn opens(raw: &[u8]) -> bool {
    opens_run(raw, raw, 0, Quotes::new())
}

/// The index of the line's plain `&`.
fn amp(line: &[u8]) -> usize {
    line.iter().position(|&b| b == b'&').unwrap()
}

/// Run `open_at_amp` with the cursor on the line's `&`, and return the segments
/// the branch pushed.
fn at_amp(line: &'static [u8], in_block: bool) -> (Vec<Segment<'static>>, usize) {
    let mut book = Book::new();
    book.i = amp(line);
    open_at_amp(line, &mut book, in_block);
    (book.segments, book.i)
}

/// Every block-opening run is recognised: the same words the separator arm
/// opens, at the same gate.
#[test]
fn every_block_opener_run_opens() {
    for raw in [
        b"case x in x) echo a & ".as_slice(),
        b"if true & ".as_slice(),
        b"for i in 1 & ".as_slice(),
        b"while : & ".as_slice(),
        b"until : & ".as_slice(),
        b"f(){ echo hi & }".as_slice(),
        b"function f { echo hi & }".as_slice(),
        b"f()(".as_slice(),
        // The `time [-p]` prefix is transparent for the scorer.
        b"time case x in x) echo a & ".as_slice(),
        // A block-opening `wait` (a pattern keyword on a later line).
        b"wait\n readable %rd) echo r ;;\ndone".as_slice(),
        // Leading whitespace is not part of the run's first word.
        b"   case x in x) echo a & ".as_slice(),
    ] {
        assert!(opens(raw), "{:?}", raw);
    }
}

/// A run that opens no block: the POSIX `wait` builtin (no pattern keyword), a
/// keyword-shaped word behind another word, a quoted keyword word (task #247), a
/// closer, and a run that is empty or blank.
#[test]
fn a_run_that_opens_nothing_is_not_a_block() {
    for raw in [
        b"wait & ".as_slice(),
        b"echo case & ".as_slice(),
        b"'case' x in x) echo a & ;; esac".as_slice(),
        b"\"if\" true & then :; fi".as_slice(),
        b"esac & ".as_slice(),
        b"echo hi &".as_slice(),
        b"".as_slice(),
        b"   ".as_slice(),
    ] {
        assert!(!opens(raw), "{:?}", raw);
    }
}

/// The plain-`&` branch of a run that opens a block pushes the block segment and
/// moves the cursor past its span, so the scan never reads the `&` written
/// inside the block's text.
#[test]
fn the_amp_boundary_opens_the_block() {
    for line in [
        b"case x in x) echo a & ;; esac".as_slice(),
        b"f(){ echo hi & }".as_slice(),
        b"if true & then echo x; fi".as_slice(),
    ] {
        let (segments, cursor) = at_amp(line, false);
        assert_eq!(segments.len(), 1, "{:?}", line);
        match &segments[0] {
            Segment::Block {
                block_start,
                closed,
                ..
            } => {
                assert_eq!(*block_start, 0, "{:?}", line);
                assert!(*closed, "{:?}", line);
            }
            Segment::Statement { cmd, .. } => {
                panic!("expected a Block, got the statement {:?}", cmd)
            }
            Segment::AsyncInvalid => panic!("unexpected async marker for {:?}", line),
        }
        assert!(
            cursor > amp(line),
            "the scan must pass the & it owns: {line:?}"
        );
    }
}

/// A run that opens nothing keeps the statement split ([`bg::mark`]), and so
/// does a keyword-leading run scanned **inside** a block: there a keyword word
/// is statement content, never a new block.
#[test]
fn a_run_that_opens_nothing_is_the_statement_split() {
    for (line, in_block) in [
        (b"echo hi &".as_slice(), false),
        (b"wait &".as_slice(), false),
        (b"case x in x) echo a & echo b".as_slice(), true),
        (b"f(){ echo hi & }".as_slice(), true),
    ] {
        let (segments, _) = at_amp(line, in_block);
        assert_eq!(segments.len(), 1, "{line:?}");
        match &segments[0] {
            Segment::Statement {
                cmd, background, ..
            } => {
                assert!(*background, "{line:?}");
                assert_eq!(*cmd, line[..amp(line)].trim_ascii(), "{line:?}");
            }
            Segment::Block { .. } => panic!("expected a Statement, not a block: {line:?}"),
            Segment::AsyncInvalid => panic!("unexpected async marker for {line:?}"),
        }
    }
}
