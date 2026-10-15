#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use alloc::vec;
use alloc::vec::Vec;

use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;
use crate::substitute::arg::substitute_arg_marked;
use crate::substitute::split::split_word;

use super::{handle, scan_body};

fn cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn peekable(bytes: &[u8]) -> core::iter::Peekable<core::iter::Copied<core::slice::Iter<'_, u8>>> {
    bytes.iter().copied().peekable()
}

/// The body scan alone: `None` when the region does not close.
fn body(input: &[u8]) -> Option<Vec<u8>> {
    scan_body(&mut peekable(input))
        .ok()
        .map(|b| b.as_bytes().unwrap().to_vec())
}

/// The arm as the pipeline runs it: the word with one mask bit per byte, and
/// the expanded text, its mask and the `expanded` flag that come back.
fn expand(word: &ShortCStr, mask: &[bool]) -> (ShortCStr, Vec<bool>, bool) {
    let cell = cell();
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    let mut expanded = false;
    let (text, out_mask) =
        substitute_arg_marked(word, mask, &mut cache, &cell, &mut expanded).unwrap();
    (text, out_mask, expanded)
}

/// The arm's output after the pipeline's `IFS` split (default `IFS`).
fn fields(word: &ShortCStr, mask: &[bool]) -> Vec<ShortCStr> {
    let (text, out_mask, expanded) = expand(word, mask);
    assert!(expanded, "the backtick arm must report an expansion");
    split_word(&text, &out_mask, &cell().borrow().unwrap().ifs)
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
    let (text, mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"hi");
    assert_eq!(mask, vec![false; 2]);
    assert!(expanded);
}

#[test]
fn unquoted_output_is_unprotected_and_splits() {
    let word = ShortCStr::from(c"x`echo a b`y");
    let (text, mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"xa by");
    assert!(expanded);
    assert_eq!(mask, vec![false; 5]);
    assert_eq!(
        fields(&word, &vec![false; word.len()]),
        [ShortCStr::from(c"xa"), ShortCStr::from(c"by"),]
    );
}

#[test]
fn quoted_output_takes_the_trigger_bit_and_never_splits() {
    let word = ShortCStr::from(c"`echo a b`");
    let (text, mask, expanded) = expand(&word, &vec![true; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"a b");
    assert!(expanded);
    assert_eq!(mask, vec![true; 3]);
    // One word whatever `IFS` is: every output byte carries a quoted bit.
    let cell = cell();
    cell.borrow_mut().unwrap().ifs = ShortCStr::from(c",");
    let ifs = cell.borrow().unwrap().ifs.clone();
    let out: Vec<ShortCStr> = split_word(&text, &mask, &ifs)
        .unwrap()
        .into_iter()
        .map(|(f, _)| f)
        .collect();
    assert_eq!(out, [ShortCStr::from(c"a b")]);
}

#[test]
fn trailing_newlines_are_stripped() {
    let word = ShortCStr::from(c"`printf \"x\\n\\n\"`");
    let (text, _mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"x");
    assert!(expanded);
}

#[test]
fn empty_body_expands_to_nothing() {
    let word = ShortCStr::from(c"``");
    let (text, mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert!(text.as_bytes().unwrap().is_empty());
    assert!(mask.is_empty());
    assert!(expanded, "an empty arm ran: POSIX 2.6.5 splitting applies");
}

#[test]
fn dollar_paren_inside_a_backtick_is_the_childs_whole_script() {
    // The body runs as the child's own text, so a body that *is* a `$( … )`
    // lands in the child's word 0 — a substitution surface (task #268): the
    // child expands it to `a`, reports the missing command `a` and prints
    // nothing, so the capture is empty and the word ends up empty. bash takes
    // the same path (`a: command not found`, empty stdout).
    let word = ShortCStr::from(c"`$(echo a)`");
    let (text, mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert!(text.as_bytes().unwrap().is_empty());
    assert!(mask.is_empty());
    assert!(expanded);
}

#[test]
fn nested_backtick_inside_a_dollar_paren_runs() {
    // The `$( … )` body is the child's text; its own backtick arg goes
    // through the child's pipeline, so the value comes back (`a`, as bash).
    let word = ShortCStr::from(c"$(echo `echo a`)");
    let (text, _mask, expanded) = expand(&word, &vec![false; word.len()]);
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
    let (text, mask, expanded) = expand(&word, &vec![false; word.len()]);
    assert_eq!(text.as_bytes().unwrap(), b"a`b");
    assert_eq!(mask, vec![false, true, false]);
    assert!(!expanded);
}
