#![allow(clippy::unwrap_used)]

use alloc::vec;
use hashbrown::HashMap;

use sys::ShortCStr;

use crate::mask::MaskBit;
use crate::substitute::substitute_arg;

use super::*;

#[test]
fn max_captured_is_64_mib() {
    assert_eq!(MAX_CAPTURED, 67_108_864);
}

fn drain_bytes(data: &[u8], limit: usize) -> Result<Vec<u8>, Report<CmdSubstError>> {
    let (r, w) = sys::pipe::pipe2(0).unwrap();
    w.write(data).unwrap();
    drop(w);
    drain(&r, limit)
}

#[test]
fn drain_strips_trailing_newlines() {
    let out = drain_bytes(b"hello\n\n", 1024).unwrap();
    assert_eq!(out, b"hello");
}

#[test]
fn drain_under_cap_returns_all() {
    let out = drain_bytes(b"abc", 3).unwrap();
    assert_eq!(out, b"abc");
}

#[test]
fn drain_over_cap_fails() {
    let result = drain_bytes(b"abcdefgh", 3);
    assert!(matches!(
        result.unwrap_err().current_context(),
        CmdSubstError::OutputTooLarge
    ));
}

#[test]
fn drain_empty_is_empty() {
    let out = drain_bytes(b"", 1024).unwrap();
    assert!(out.is_empty());
}

/// The reap site **keeps** the child's status in the state's scratch slot — the
/// only way a word-0 `$(false)` can report 1 (task #426, POSIX #2.9.1). It is the
/// shared path of both spellings, so each spelling answers through it, and the
/// child exits with its script's status for the recording to mean anything.
#[test]
fn the_reap_records_the_substitution_status() {
    for (name, want) in [
        ("$(false)", 1),
        ("`false`", 1),
        ("$(true)", 0),
        ("`true`", 0),
        ("$(exit 3)", 3),
    ] {
        let cell = ForkCell::new(ShellState::new());
        let mut cache = HashMap::new();
        let word = ShortCStr::from_vec(name.as_bytes().to_vec()).unwrap();
        let mask = vec![MaskBit::Unquoted; word.len()];
        substitute_arg(&word, &mask, &[], &mut cache, &cell).unwrap();
        assert_eq!(
            cell.borrow()
                .unwrap()
                .last_substitution
                .as_ref()
                .map(|s| s.exit_code()),
            Some(want),
            "{name} must reach the state through the shared reap"
        );
    }
}
