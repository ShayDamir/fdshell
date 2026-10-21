#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::vec;
use alloc::vec::Vec;

use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::mask::MaskBit;
use crate::mask::MaskBit::{Quoted, Unquoted};
use crate::state::ShellState;
use crate::substitute::arg::substitute_arg_marked;
use crate::substitute::mask::Counting;
use crate::substitute::origin::Origin;
use crate::substitute::split::split_word;

use super::{handle, scan_body};

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn peekable(bytes: &[u8]) -> Counting<'_> {
    Counting::new(bytes.iter().copied())
}

/// The body scan alone: `None` when the region does not close.
fn body(input: &[u8]) -> Option<Vec<u8>> {
    scan_body(&mut peekable(input))
        .ok()
        .map(|b| b.as_bytes().unwrap().to_vec())
}

/// The arm as the pipeline runs it: the word with one mask bit per byte, and
/// the expanded text, its mask and the splitting signal (`true` exactly when
/// the arm produced splittable bytes — the provenance of task #222,
/// superseding the `expanded` flag of task #133).
fn expand(word: &ShortCStr, mask: &[MaskBit]) -> (ShortCStr, Vec<MaskBit>, bool) {
    let cell = cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (text, out_mask, origin) =
        substitute_arg_marked(word, mask, &[], &mut cache, &cell).unwrap();
    (text, out_mask, !origin.is_empty())
}

/// The arm's output after the pipeline's `IFS` split (default `IFS`).
fn fields(word: &ShortCStr, mask: &[MaskBit]) -> Vec<ShortCStr> {
    let cell = cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let (text, out_mask, origin) =
        substitute_arg_marked(word, mask, &[], &mut cache, &cell).unwrap();
    assert!(
        !origin.is_empty(),
        "the backtick arm must produce splittable bytes"
    );
    split_word(&text, &out_mask, &cell.borrow().unwrap().ifs, &origin)
        .unwrap()
        .into_iter()
        .map(|(field, _)| field)
        .collect()
}

#[test]
fn scan_body_reads_up_to_the_closing_backtick() {
    assert_eq!(body(b"echo hi`rest"), Some(b"echo hi".to_vec()));
    assert_eq!(body(b"`"), Some(Vec::new()));
    // Quote bytes are body content: the child re-parses them quote-aware.
    assert_eq!(body(b"echo \"a b\"`"), Some(b"echo \"a b\"".to_vec()));
    assert_eq!(body(b"echo $(echo a)`"), Some(b"echo $(echo a)".to_vec()));
}

#[test]
fn scan_body_keeps_escape_pairs_verbatim() {
    // The backslash shields the closer in the scan and reaches the child.
    assert_eq!(body(b"echo \\`x`"), Some(b"echo \\`x".to_vec()));
    assert_eq!(body(b"echo \\\\`"), Some(b"echo \\\\".to_vec()));
    // An escaped backtick is body data, never the closer: the region left
    // open after it is the unclosed-region error.
    assert_eq!(body(b"echo a\\`"), None);
}

#[test]
fn scan_body_reports_an_unclosed_region() {
    for input in [b"echo hi".as_slice(), b"echo \\".as_slice()] {
        let mut peek = peekable(input);
        assert!(
            matches!(
                scan_body(&mut peek).err().unwrap().current_context(),
                ResolveError::UnclosedBacktick,
            ),
            "{input:?}"
        );
    }
}

#[test]
fn scan_body_reports_a_nul_byte() {
    let mut peek = peekable(b"a\x00b`");
    assert!(matches!(
        scan_body(&mut peek).err().unwrap().current_context(),
        ResolveError::NulByte,
    ));
}

#[test]
fn whole_word_output_is_the_body_result() {
    let word = ShortCStr::from(c"`echo hi`");
    let (text, mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"hi");
    assert_eq!(mask, vec![Unquoted; 2]);
    assert!(expanded);
}

#[test]
fn unquoted_output_is_unprotected_and_splits() {
    let word = ShortCStr::from(c"x`echo a b`y");
    let (text, mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"xa by");
    assert!(expanded);
    assert_eq!(mask, vec![Unquoted; 5]);
    assert_eq!(
        fields(&word, &vec![Unquoted; word.len()]),
        [ShortCStr::from(c"xa"), ShortCStr::from(c"by"),]
    );
}

#[test]
fn quoted_output_takes_the_trigger_bit_and_never_splits() {
    let word = ShortCStr::from(c"`echo a b`");
    let (text, mask, expanded) = expand(&word, &vec![Quoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"a b");
    assert!(expanded);
    assert_eq!(mask, vec![Quoted; 3]);
    // One word whatever `IFS` is: every output byte carries a quoted bit.
    let cell = cell();
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    let ifs = cell.borrow().unwrap().ifs.clone();
    let out: Vec<ShortCStr> = split_word(&text, &mask, &ifs, &Origin::all(text.len()))
        .unwrap()
        .into_iter()
        .map(|(f, _)| f)
        .collect();
    assert_eq!(out, [ShortCStr::from(c"a b")]);
}

#[test]
fn trailing_newlines_are_stripped() {
    let word = ShortCStr::from(c"`printf \"x\\n\\n\"`");
    let (text, _mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"x");
    assert!(expanded);
}

#[test]
fn empty_body_records_no_splittable_bytes() {
    let word = ShortCStr::from(c"``");
    let (text, mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert!(text.as_bytes().unwrap().is_empty());
    assert!(mask.is_empty());
    // Task #222: an arm that produced no bytes has nothing to split; the
    // word-level rule (the empty unquoted word is no word, the quoted-empty
    // word survives) is pinned in `tests/wordsplit.rs`.
    assert!(!expanded);
}

#[test]
fn dollar_paren_inside_a_backtick_is_the_childs_whole_script() {
    // The body runs as the child's own text, so a body that *is* a `$( … )`
    // lands in the child's word 0 — a substitution surface (task #268): the
    // child expands it to `a`, reports the missing command `a` and prints
    // nothing, so the capture is empty and the word ends up empty. bash takes
    // the same path (`a: command not found`, empty stdout).
    let word = ShortCStr::from(c"`$(echo a)`");
    let (text, mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert!(text.as_bytes().unwrap().is_empty());
    assert!(mask.is_empty());
    assert!(!expanded, "no bytes, nothing splittable (task #222)");
}

#[test]
fn nested_backtick_inside_a_dollar_paren_runs() {
    // The `$( … )` body is the child's text; its own backtick arg goes
    // through the child's pipeline, so the value comes back (`a`, as bash).
    let word = ShortCStr::from(c"$(echo `echo a`)");
    let (text, _mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"a");
    assert!(expanded);
}

#[test]
fn handle_reports_the_capture_limit() {
    // The arm inherits `cmd_subst`'s cap: an oversized body output fails
    // cleanly, it is never captured and never grows the shell.
    let cell = cell();
    cell.borrow_mut().unwrap().capture_limit = 4;
    let mut out = ShortCStr::new();
    let mut out_mask = Vec::new();
    let mut peek = peekable(b"`printf abcdefgh`");
    peek.next();
    assert!(matches!(
        handle(&mut peek, &cell, &mut out, &mut out_mask, false)
            .err()
            .unwrap()
            .current_context(),
        ResolveError::Resolve,
    ));
}

#[test]
fn escaped_backtick_is_data_not_an_arm() {
    // `\<backtick>` is taken by the escape arm before the backtick arm could
    // see it: the byte stays literal and its mask bit is the escape's `true`.
    let word = ShortCStr::from(c"a\\`b");
    let (text, mask, expanded) = expand(&word, &vec![Unquoted; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"a`b");
    assert_eq!(mask, vec![Unquoted, Quoted, Unquoted]);
    assert!(!expanded);
}
