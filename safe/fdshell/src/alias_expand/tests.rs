#![allow(clippy::unwrap_used)]
use super::expand_alias;
use crate::state::ShellState;
use alloc::vec::Vec;
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText, ShortCStr};

fn from(s: &str) -> ShortCStr {
    ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap()
}

/// Expand `line` with the given `name=value` aliases and return the bytes.
fn run(line: &str, aliases: &[(&str, &str)]) -> Vec<u8> {
    let cell = ForkCell::new({
        let mut state = ShellState::new();
        for (k, v) in aliases {
            state.aliases.insert(from(k), from(v));
        }
        state
    });
    let text = ScriptText::new(from(line), Position::new(1, 1), Origin::Stdin);
    let out = expand_alias(&text, &cell).unwrap();
    out.as_bytes().unwrap().to_vec()
}

#[test]
fn shrinking_alias_replaces_word() {
    assert_eq!(run("ab now", &[("ab", "z")]), b"z now");
}

#[test]
fn empty_alias_removes_word() {
    assert_eq!(run("x now", &[("x", "")]), b" now");
}

#[test]
fn chained_shrinking_alias() {
    assert_eq!(run("a now", &[("a", "b"), ("b", "c")]), b"c now");
}

#[test]
fn shrink_shifts_later_pipeline_word() {
    assert_eq!(
        run("ls | grep x", &[("ls", "l"), ("grep", "g")]),
        b"l | g x"
    );
}

#[test]
fn growing_alias_still_replaces_word() {
    assert_eq!(run("l now", &[("l", "ls")]), b"ls now");
}

#[test]
fn unrelated_word_unchanged() {
    assert_eq!(run("echo hi", &[("ab", "z")]), b"echo hi");
}

#[test]
fn drift_accumulates_across_three_positions() {
    assert_eq!(
        run("a | b | c", &[("a", "xx"), ("b", "yy"), ("c", "z")]),
        b"xx | yy | z"
    );
}

#[test]
fn partially_quoted_word_not_expanded() {
    assert_eq!(run("ab\"x\" now", &[("abx", "z")]), b"ab\"x\" now");
}

fn aliased_cell() -> ForkCell<ShellState> {
    ForkCell::new({
        let mut state = ShellState::new();
        state.aliases.insert(from("w"), from("v"));
        state
    })
}

#[test]
fn negative_offset_maps_to_never() {
    let mut cur = from("abc");
    let mut delta: isize = -20;
    assert!(
        super::expand_at::expand_at(&mut cur, &mut delta, from("w"), 5, 7, &aliased_cell())
            .is_err()
    );
}

#[test]
fn out_of_range_end_maps_to_never() {
    let mut cur = from("abc");
    let mut delta: isize = 0;
    assert!(
        super::expand_at::expand_at(&mut cur, &mut delta, from("w"), 1, 10, &aliased_cell())
            .is_err()
    );
}
