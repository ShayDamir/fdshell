#![allow(clippy::unwrap_used)]

use super::*;
use crate::intercept::try_intercept;
use crate::parse::{BuiltinPrefix, CommandLine};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::{ImportedStr, Origin, Position, ShortCStr};

fn s(text: &str) -> ShortCStr {
    ShortCStr::from_vec(text.as_bytes().to_vec()).unwrap()
}

fn bytes(word: &ShortCStr) -> Vec<u8> {
    word.as_bytes().unwrap().to_vec()
}

/// A `hash` command line with **real** masks: `quoted` sets every mask bit of
/// every word, the raw form clears them. Never an empty mask — an empty mask
/// reads as fully quoted and silently disables the split and glob rows
/// (LESSONS "No substitution surface takes an empty mask").
fn cmdline(args: &[&str], quoted: bool) -> CommandLine {
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: c"hash".into(),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args.iter().map(|a| s(a)).collect(),
        args_mask: args.iter().map(|a| vec![quoted; a.len()]).collect(),
        args_span: vec![vec![]; args.len()],
        args_quoted: vec![quoted; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn line(args: &[&str]) -> Vec<u8> {
    if args.is_empty() {
        b"hash".to_vec()
    } else {
        format!("hash {}", args.join(" ")).into_bytes()
    }
}

fn run(args: &[&str], cell: &ForkCell<ShellState>) -> Result<bool, Report<CmdError>> {
    run_hash(&line(args), &cmdline(args, false), cell)
}

fn set_var(cell: &ForkCell<ShellState>, name: &str, value: &str) {
    cell.borrow_mut()
        .unwrap()
        .set_var(s(name), ImportedStr::shell(s(value)));
}

/// The table's entry for `name`, if any.
fn entry(cell: &ForkCell<ShellState>, name: &str) -> Option<Vec<u8>> {
    let state = cell.borrow().unwrap();
    state.hash_table.get(&s(name)).map(bytes)
}

/// The status `hash` left in the state (the rc the shell reports).
fn status(cell: &ForkCell<ShellState>) -> i32 {
    cell.borrow().unwrap().last_status.exit_code()
}

/// The table key is the **expanded** name, the same shape word 0's expansion
/// stores (`prehash`), so `hash $n` and `$n` meet on the key `ls`.
#[test]
fn parameter_expansion_names_the_key() {
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "n", "ls");
    run(&["$n", "/bin/true"], &cell).unwrap();
    assert_eq!(entry(&cell, "ls").as_deref(), Some(b"/bin/true".as_slice()));
    assert!(
        entry(&cell, "$n").is_none(),
        "the raw word must not be the key"
    );
}

/// The escape pair folds inside the substitution: the key is `a*`, and the
/// folded `*` is mask-protected, so no glob runs at the CWD.
#[test]
fn escape_pair_folds_into_the_key() {
    let cell = ForkCell::new(ShellState::new());
    run(&[r"a\*", "/tmp/pinned"], &cell).unwrap();
    assert_eq!(
        entry(&cell, "a*").as_deref(),
        Some(b"/tmp/pinned".as_slice())
    );
}

/// A fully quoted word is one literal field: no split, no glob, and the
/// `*` stays a pattern byte in the key.
#[test]
fn quoted_name_is_the_literal_key() {
    let cell = ForkCell::new(ShellState::new());
    run_hash(
        &line(&["a*", "/tmp/pinned"]),
        &cmdline(&["a*", "/tmp/pinned"], true),
        &cell,
    )
    .unwrap();
    assert_eq!(
        entry(&cell, "a*").as_deref(),
        Some(b"/tmp/pinned".as_slice())
    );
}

/// A command substitution names the key, and the option word is read **after**
/// the expansion: `hash $(echo -r)` clears the table.
#[test]
fn command_substitution_expands_the_name_and_the_option() {
    let cell = ForkCell::new(ShellState::new());
    run(&["ls", "/bin/true"], &cell).unwrap();
    assert!(entry(&cell, "ls").is_some());
    run(&["$(echo -r)"], &cell).unwrap();
    assert_eq!(status(&cell), 0);
    assert!(
        entry(&cell, "ls").is_none(),
        "the expanded option word must clear the table"
    );
}

/// 0 fields is a bare listing, not an empty name: `hash $@` with no
/// positionals (and an unset `$zz`) writes no key and exits 0.
#[test]
fn zero_fields_is_a_bare_listing() {
    let cell = ForkCell::new(ShellState::new());
    for spelling in ["$@", "$zz"] {
        run(&[spelling], &cell).unwrap();
        assert_eq!(status(&cell), 0, "{spelling}");
        assert!(
            cell.borrow().unwrap().hash_table.is_empty(),
            "{spelling} must not write a key"
        );
    }
}

/// The bare listing with entries present runs to completion and exits 0 (its
/// `name<TAB>path` bytes land on the test's own stdout, captured by nextest;
/// the listing format is pinned at integration level by
/// `tests/hash.rs::hash_pins_and_lists`).
#[test]
fn bare_hash_lists_a_non_empty_table() {
    let cell = ForkCell::new(ShellState::new());
    run(&["a", "/tmp/a"], &cell).unwrap();
    run(&["b", "/tmp/b"], &cell).unwrap();
    run(&[], &cell).unwrap();
    assert_eq!(status(&cell), 0);
    assert_eq!(entry(&cell, "a").as_deref(), Some(b"/tmp/a".as_slice()));
}

/// `hash -r name…` (fdshell's extension) removes the **expanded** name.
#[test]
fn dash_r_removes_the_expanded_name() {
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "n", "ls");
    run(&["ls", "/bin/true"], &cell).unwrap();
    run(&["-r", "$n"], &cell).unwrap();
    assert_eq!(entry(&cell, "ls"), None);
    assert_eq!(status(&cell), 0);
}

/// `hash -d name` removes a present entry silently, rc 0.
#[test]
fn dash_d_removes_a_present_name() {
    let cell = ForkCell::new(ShellState::new());
    set_var(&cell, "n", "ls");
    run(&["ls", "/bin/true"], &cell).unwrap();
    run(&["-d", "$n"], &cell).unwrap();
    assert_eq!(entry(&cell, "ls"), None);
    assert_eq!(status(&cell), 0);
}

/// `hash -d nosuch` reports bash's `not found` shape and exits 1; a name list
/// of several names exits 1 when any of them is missing.
#[test]
fn dash_d_reports_a_missing_name_and_exits_one() {
    let cell = ForkCell::new(ShellState::new());
    run(&["ls", "/bin/true"], &cell).unwrap();
    run(&["-d", "ls", "nosuch"], &cell).unwrap();
    assert_eq!(entry(&cell, "ls"), None, "the present name is removed");
    assert_eq!(status(&cell), 1);
    run(&["-d", "nosuch"], &cell).unwrap();
    assert_eq!(status(&cell), 1);
}

/// A bare `hash -d` is fdshell's usage error (bash's getopt diagnostic, rc 1).
#[test]
fn dash_d_without_a_name_is_usage() {
    let cell = ForkCell::new(ShellState::new());
    let report = run(&["-d"], &cell);
    assert!(matches!(
        report.unwrap_err().current_context(),
        CmdError::HashUsage
    ));
}

/// A third field is fdshell's usage error, not a third name (bash hashes each
/// name; the option/list surface is task #375).
#[test]
fn three_fields_is_usage_error() {
    let cell = ForkCell::new(ShellState::new());
    let report = run(&["a", "b", "c"], &cell);
    assert!(matches!(
        report.unwrap_err().current_context(),
        CmdError::HashUsage
    ));
}

/// `hash name path` pins, `hash -r` clears — the pair that ran on the raw
/// words keeps working, and their **empty** masks are the pin that an empty
/// mask stays a fully quoted word (no split, no glob).
#[test]
fn pin_and_clear_through_the_intercept() {
    let cell = ForkCell::new(ShellState::new());
    let args = ["mybin", "/bin/mybin"];
    let mut plain = cmdline(&args, false);
    plain.args_mask = vec![vec![]; args.len()];
    plain.args_quoted = vec![false; args.len()];
    let text = sys::ScriptText::new(
        s("hash mybin /bin/mybin"),
        Position::new(1, 1),
        Origin::Shell,
    );
    assert!(
        try_intercept(&text, &plain, &cell).unwrap().is_some(),
        "hash name path must be handled"
    );
    assert_eq!(
        entry(&cell, "mybin").as_deref(),
        Some(b"/bin/mybin".as_slice())
    );
    let clear = cmdline(&["-r"], false);
    let text = sys::ScriptText::new(s("hash -r"), Position::new(1, 1), Origin::Shell);
    assert!(try_intercept(&text, &clear, &cell).unwrap().is_some());
    assert!(
        cell.borrow().unwrap().hash_table.is_empty(),
        "the table must be empty"
    );
}
