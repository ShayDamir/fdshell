//! Tests for the conditional part runner.

#![allow(clippy::unwrap_used)]

use super::reconstruct;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// A part with bodies is reconstructed as the command bytes plus a newline
/// plus the bodies (in order), positioned at the part's start.
#[test]
fn reconstruct_appends_newline_and_bodies() {
    let line = b"cat <<A && cat <<B\nbodyA\nA\nbodyB\nB";
    let text = stext(line);
    // Part 1: `cat <<A` (0..8), body `bodyA\nA\n`.
    // The part bytes keep their original trailing whitespace (the space
    // before the `&&`), then a newline, then the body.
    let out = reconstruct(&text, line, 0, 8, &[b"bodyA\nA\n".to_vec()]).unwrap();
    assert_eq!(out.as_bytes().unwrap(), b"cat <<A \nbodyA\nA\n");
    // Part 2: `cat <<B` (11..18), body `bodyB\nB`.
    let out = reconstruct(&text, line, 11, 18, &[b"bodyB\nB".to_vec()]).unwrap();
    assert_eq!(out.as_bytes().unwrap(), b"cat <<B\nbodyB\nB");
}

/// A part with no bodies is not reconstructed (the caller subslices).
#[test]
fn reconstruct_empty_bodies_is_command_only() {
    let line = b"echo hello";
    let text = stext(line);
    let out = reconstruct(&text, line, 0, 10, &[]).unwrap();
    assert_eq!(out.as_bytes().unwrap(), b"echo hello\n");
}
