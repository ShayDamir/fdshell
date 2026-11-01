#![allow(clippy::unwrap_used)]
use super::*;
use crate::capture::Capture;
use crate::intercept::try_intercept;
use crate::mask::MaskBit;
use crate::parse::{BuiltinPrefix, CommandLine};
use crate::state::frames::{pop_frame, push_frame};
use alloc::vec;
use alloc::vec::Vec;
use sys::{ImportedStr, Origin, Position, ShortCStr};

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

fn text(b: &[u8]) -> ScriptText {
    ScriptText::new(s(b), Position::new(1, 1), Origin::Shell)
}

fn make_line(args: &[&str]) -> Vec<u8> {
    alloc::format!("local {}", args.join(" ")).into_bytes()
}

/// `CommandLine` for `local` words, with a per-word quote mask of the given
/// kind: the mask is parallel to its word (`args_mask[i]` describes
/// `args[i]`), so quoting every byte means `quoted = true`, unquoted `false`.
fn make_cmdline_with_masks(args: &[&str], quoted: bool) -> CommandLine {
    CommandLine {
        prefix: BuiltinPrefix::None,
        command: c"local".into(),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args.iter().map(|a| s(a.as_bytes())).collect(),
        args_mask: args
            .iter()
            .map(|a| vec![MaskBit::from_quoted(quoted); a.len()])
            .collect(),
        args_span: vec![vec![]; args.len()],
        args_quoted: vec![quoted; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cmdline(args: &[&str]) -> CommandLine {
    make_cmdline_with_masks(args, false)
}

/// A cell inside a function call (the `local` frame `function_call` pushes).
fn make_cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    push_frame(&cell).unwrap();
    cell
}

fn run(
    args: &[&str],
    cell: &ForkCell<ShellState>,
) -> Result<Option<Vec<ShortCStr>>, Report<CmdError>> {
    run_local(&make_line(args), &make_cmdline(args), &text(b"local"), cell)
}

fn set_caller_var(cell: &ForkCell<ShellState>, name: &ShortCStr, value: &ShortCStr) {
    let mut state = cell.borrow_mut().unwrap();
    state.set_var(name.clone(), ImportedStr::shell(value.clone()));
}

fn stored(cell: &ForkCell<ShellState>, name: &ShortCStr) -> Option<ShortCStr> {
    let state = cell.borrow().unwrap();
    state.strings.get(name).map(|v| v.value.clone())
}

fn exported(cell: &ForkCell<ShellState>, name: &ShortCStr) -> Option<ShortCStr> {
    let state = cell.borrow().unwrap();
    state.exports.get(name).map(|v| v.value.clone())
}

#[test]
fn local_outside_a_frame_is_rejected() {
    let cell = ForkCell::new(ShellState::new());
    let report = run(&["v=1"], &cell).unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::LocalOutsideFunction
    ));
}

#[test]
fn assignment_shadows_and_stores() {
    let cell = make_cell();
    set_caller_var(&cell, &s(b"v"), &s(b"pre"));
    assert!(run(&["v=1"], &cell).unwrap().is_some());
    assert_eq!(stored(&cell, &s(b"v")), Some(s(b"1")));
    let state = cell.borrow().unwrap();
    assert_eq!(state.local_names().len(), 1);
    assert_eq!(state.last_status.exit_code(), 0);
}

#[test]
fn assignment_is_exported_inside_the_call() {
    let cell = make_cell();
    assert!(run(&["E=loc"], &cell).unwrap().is_some());
    assert_eq!(exported(&cell, &s(b"E")), Some(s(b"loc")));
}

#[test]
fn declare_form_leaves_the_name_unset() {
    let cell = make_cell();
    set_caller_var(&cell, &s(b"v"), &s(b"pre"));
    assert!(run(&["v"], &cell).unwrap().is_some());
    assert_eq!(stored(&cell, &s(b"v")), None);
    // The export attribute stays global (bash parity), so `exports` is untouched.
    assert_eq!(exported(&cell, &s(b"v")), None);
    assert_eq!(cell.borrow().unwrap().local_names().len(), 1);
}

#[test]
fn several_words_shadow_in_one_call() {
    let cell = make_cell();
    assert!(run(&["p=1", "q=2"], &cell).unwrap().is_some());
    assert_eq!(stored(&cell, &s(b"p")), Some(s(b"1")));
    assert_eq!(stored(&cell, &s(b"q")), Some(s(b"2")));
    assert_eq!(cell.borrow().unwrap().local_names().len(), 2);
}

#[test]
fn bad_names_are_rejected() {
    let cell = make_cell();
    for word in ["%x=1", "=1", "-x", "-r=v"] {
        let report = run(&[word], &cell).unwrap_err();
        assert!(
            matches!(report.current_context(), CmdError::LocalBadName { .. }),
            "{word} must be rejected"
        );
    }
    // The rejected word leaves nothing behind (the name is checked before any store).
    let state = cell.borrow().unwrap();
    assert!(state.local_names().is_empty());
}

#[test]
fn value_is_expanded_without_splitting_or_glob() {
    let cell = make_cell();
    set_caller_var(&cell, &s(b"v"), &s(b"a b"));
    assert!(run(&["x=*", "y=$v"], &cell).unwrap().is_some());
    assert_eq!(stored(&cell, &s(b"x")), Some(s(b"*")));
    assert_eq!(stored(&cell, &s(b"y")), Some(s(b"a b")));
}

#[test]
fn captures_are_rejected() {
    let cell = make_cell();
    let mut cmdline = make_cmdline(&["v=1"]);
    cmdline.captures = vec![Capture {
        var: s(b"fd"),
        tag: None,
        force: false,
        cap: None,
        set_at: Position::new(1, 1),
    }];
    let report = run_local(&make_line(&["v=1"]), &cmdline, &text(b"local v=1"), &cell).unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::CapturesNotSupported { .. }
    ));
}

#[test]
fn list_form_is_handled_and_keeps_the_status_zero() {
    let cell = make_cell();
    assert!(run(&[], &cell).unwrap().is_some());
    assert_eq!(cell.borrow().unwrap().last_status.exit_code(), 0);
}

#[test]
fn the_list_form_prints_the_shadowed_names_with_their_values() {
    // Prints `local p=1` and `local q=2` (sorted by bytes) on stdout; the
    // in-process assertions cover the state the print path must leave behind.
    let cell = make_cell();
    assert!(run(&["q=2", "p=1"], &cell).unwrap().is_some());
    assert!(run(&[], &cell).unwrap().is_some());
    assert_eq!(cell.borrow().unwrap().last_status.exit_code(), 0);
    assert_eq!(stored(&cell, &s(b"p")), Some(s(b"1")));
    assert_eq!(stored(&cell, &s(b"q")), Some(s(b"2")));
}

#[test]
fn the_list_form_prints_a_declared_name_without_a_value() {
    // `local p` prints as `local p` (the POSIX/dash form, an accepted
    // divergence from bash's `declare -- p="..."`), leaving the name unset.
    let cell = make_cell();
    set_caller_var(&cell, &s(b"p"), &s(b"pre"));
    assert!(run(&["p"], &cell).unwrap().is_some());
    assert!(run(&[], &cell).unwrap().is_some());
    assert_eq!(cell.borrow().unwrap().last_status.exit_code(), 0);
    assert_eq!(stored(&cell, &s(b"p")), None);
    {
        // The declare form touches nothing outside `strings`, so a caller value
        // that was never exported stays unexported (bash parity).
        let state = cell.borrow().unwrap();
        assert_eq!(state.exports.get(&s(b"p")).map(|v| v.value.clone()), None);
    }
}

#[test]
fn local_ifs_syncs_the_splitting_and_is_recorded() {
    let cell = make_cell();
    let caller_ifs = cell.borrow().unwrap().ifs.clone();
    assert!(run(&["IFS=:"], &cell).unwrap().is_some());
    {
        let state = cell.borrow().unwrap();
        assert_eq!(state.ifs, s(b":"));
    }
    pop_frame(&cell).unwrap();
    assert_eq!(cell.borrow().unwrap().ifs, caller_ifs);
}

/// Task #217: the value is expanded with the **value word's own** quote mask —
/// the word mask sliced at the `=` (the name's length is the offset). The two
/// runs are the same bytes with the two masks, so each kills the other's
/// mutant: an empty mask (the old `&[]`) fails the quoted run, a
/// neighbour-mispliced mask fails the unquoted one.
#[test]
fn local_value_keeps_a_quoted_escape_pair() {
    let word = "q=a\\ b";
    let line = make_line(&[word]);

    let cell = make_cell();
    let quoted = make_cmdline_with_masks(&[word], true);
    run_local(&line, &quoted, &text(b"local"), &cell).unwrap();
    assert_eq!(stored(&cell, &s(b"q")), Some(s(b"a\\ b")));

    let cell = make_cell();
    let plain = make_cmdline_with_masks(&[word], false);
    run_local(&line, &plain, &text(b"local"), &cell).unwrap();
    assert_eq!(stored(&cell, &s(b"q")), Some(s(b"a b")));
}

/// Run `local` through `try_intercept`, so the `$_` frame is part of the run
/// (the direct `run` helper above stops short of it).
fn through_intercept(args: &[&str], cell: &ForkCell<ShellState>) {
    let line = make_line(args);
    assert!(
        try_intercept(&text(&line), &make_cmdline(args), cell)
            .unwrap()
            .is_some()
    );
}

/// Task #376: `$_` is the word `local` **consumed** — the name with the expanded
/// value, as bash sets it. The rebuild is byte-identical to the raw word when
/// nothing expanded, and a bare name is reported as-is.
#[test]
fn last_arg_is_the_consumed_word() {
    let cell = make_cell();
    set_caller_var(&cell, &s(b"V"), &s(b"hello"));
    through_intercept(&[r"x=$V"], &cell);
    assert_eq!(stored(&cell, &s(b"_")), Some(s(b"x=hello")));

    let cell = make_cell();
    through_intercept(&["plain=x"], &cell);
    assert_eq!(stored(&cell, &s(b"_")), Some(s(b"plain=x")));

    let cell = make_cell();
    through_intercept(&["bare"], &cell);
    assert_eq!(stored(&cell, &s(b"bare")), None, "the declare form");
    assert_eq!(stored(&cell, &s(b"_")), Some(s(b"bare")));
}
