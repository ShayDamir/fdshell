//! Tests for the condition body extraction.

#![allow(clippy::unwrap_used)]

use super::condition_bodies;
use crate::parse::semi::{find_preceded_by_semi, trim_semi};
use crate::parse::token::tokenize_statement;
use alloc::vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

/// The condition's bodies (body lines plus the delimiter line) are extracted
/// from the block text, which carries them after the closing keyword.
#[test]
fn condition_bodies_extracted_from_block_text() {
    let line = b"if cat <<EOF; then echo yes; fi\nbody\nEOF";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    let bodies = condition_bodies(&text, cond_tokens);
    assert_eq!(
        bodies,
        vec![ShortCStr::from_vec(b"body\nEOF".to_vec()).unwrap()]
    );
}

/// A condition without `<<` operators yields no bodies.
#[test]
fn condition_bodies_no_operators_is_empty() {
    let line = b"if true; then echo yes; fi\nbody\nEOF";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    assert!(condition_bodies(&text, cond_tokens).is_empty());
}

/// Two operators in the condition yield two bodies in operator order.
#[test]
fn condition_bodies_multiple_operators_are_in_order() {
    let line = b"if cat <<A && cat <<B; then fi\nbodyA\nA\nbodyB\nB";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    let bodies = condition_bodies(&text, cond_tokens);
    assert_eq!(
        bodies,
        vec![
            ShortCStr::from_vec(b"bodyA\nA\n".to_vec()).unwrap(),
            ShortCStr::from_vec(b"bodyB\nB".to_vec()).unwrap(),
        ]
    );
}
