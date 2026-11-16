#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use sys::ImportedStr;
use sys::fork_cell::ForkCell;
use sys::{Origin, Position, ScriptText};

fn make_cmdline() -> crate::parse::CommandLine {
    cmdline_with(&[b"a", b"o"])
}

/// The same inert `CommandLine`, with the argument list (optstring, name, then
/// the `arg…` operands) as given.
fn cmdline_with(args: &[&[u8]]) -> crate::parse::CommandLine {
    crate::parse::CommandLine {
        prefix: crate::parse::BuiltinPrefix::None,
        command: ShortCStr::from(c"getopts"),
        command_mask: vec![],
        command_span: vec![],
        command_quoted: false,
        env_assigns: vec![],
        args: args.iter().map(|a| s(a)).collect(),
        args_mask: args.iter().map(|_| vec![]).collect(),
        args_span: args.iter().map(|_| vec![]).collect(),
        args_quoted: args.iter().map(|_| false).collect(),
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// A fresh shell with the positional parameters seeded through the store's own
/// setter path (`set --`), plus a starting counter/cursor when asked.
fn seeded(positional: &[&[u8]]) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    let pos: VecDeque<ImportedStr> = positional
        .iter()
        .map(|p| ImportedStr::shell(s(p)))
        .collect();
    cell.borrow_mut().unwrap().set_positional(pos);
    cell
}

fn run(cell: &ForkCell<ShellState>, args: &[&[u8]]) -> (i32, u64, usize) {
    let cmdline = cmdline_with(args);
    let handled = run_getopts(b"getopts ab:c o", &cmdline, cell).unwrap();
    assert!(
        handled,
        "getopts is claimed (the launch path never sees it)"
    );
    let state = cell.borrow().unwrap();
    (state.last_status.exit_code(), state.optind, state.optchar)
}

/// The counter's default is **1** — the value bash 5.3.9 measures at startup, and
/// the value `unset OPTIND` falls back to (task #589).
#[test]
fn the_default_counter_is_one() {
    assert_eq!(DEFAULT_OPTIND, 1);
}

/// Only an **exact** decimal moves the counter; a value with no digits at all
/// leaves it where it was (bash measures `OPTIND=abc` as "start the scan at 1"
/// while `$OPTIND` prints `abc`). The arithmetic-evaluator forms are #590's.
#[test]
fn only_an_exact_parse_moves_the_counter() {
    assert_eq!(counter(&ShortCStr::from(c"3"), DEFAULT_OPTIND), 3);
    assert_eq!(counter(&ShortCStr::from(c"abc"), 4), 4);
    assert_eq!(counter(&ShortCStr::from(c""), 4), 4);
}

/// The scan claims the command (`true`, so `run/parent.rs` never resolves a path
/// for it) — **this is the pin moved from the #589 stub**: `run_getopts` with no
/// operands ends the scan with bash's status 1 at OPTIND 1 (`$#`+1 of an empty
/// list), naming `?` and leaving `OPTARG` unbound (bash 5.3.9, re-measured on
/// task591; the write is task #591's, so the pin covers all of it now).
#[test]
fn run_getopts_is_handled_and_the_empty_scan_ends_at_the_seed() {
    let cell = ForkCell::new(ShellState::new());
    // Seed a zero status so the assertion below is not tautological (the
    // `colon` tests' shape): the handler must report bash's end status 1.
    cell.borrow_mut().unwrap().set_last_exit(0);
    let cmdline = make_cmdline();
    assert!(run_getopts(b"getopts a o", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(
        state.last_status.exit_code(),
        1,
        "the end-of-options status"
    );
    assert_eq!(state.optind, DEFAULT_OPTIND, "OPTIND stays at $# + 1");
    drop(state);
    assert_eq!(
        called(&cell, &[b"a", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 1, 0, 1),
        "the same call, read back through the caller's variables"
    );
}

#[test]
fn the_operand_list_is_the_word_list_when_present() {
    // Measured `getopts "ab:c" o -a -b val extra`: the operand list is the word
    // list and OPTIND counts indices into it — the seeded positionals (`x`) would
    // end the scan at 1 with status 1 instead.
    let cell = seeded(&[b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o", b"-a", b"val"]);
    assert_eq!(
        (code, index, cursor),
        (0, 2, 0),
        "the operand `-a` was scanned"
    );
}

#[test]
fn the_positional_parameters_are_the_word_list_otherwise() {
    // Measured `set -- -a b c`: `a` reports at OPTIND 2 (status 0).
    let cell = seeded(&[b"-a", b"b", b"c"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 2, 0));
    // The measured continuation: the walk stops on `b` — end of options, status
    // 1, OPTIND stays 2.
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 2, 0));
}

#[test]
fn a_flag_never_consumes_the_next_word_and_advances_one_index() {
    // Measured `set -- -c val`: `c` reports, OPTIND 2 — `val` stands free; a
    // flag that consumed it would answer 3.
    let cell = seeded(&[b"-c", b"val"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 2, 0));
}

#[test]
fn an_attached_argument_and_the_cluster_resume_across_calls() {
    // Measured `set -- -abc val`: `a` with OPTIND staying 1 mid-word, then `b`
    // taking `c` (the attached argument) and landing on 2.
    let cell = seeded(&[b"-abc", b"val"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!(
        (code, index, cursor),
        (0, 1, 2),
        "a reports, cursor on the word"
    );
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 2, 0), "b takes the attached `c`");
}

#[test]
fn a_cluster_letter_with_nothing_left_takes_the_next_word_whole() {
    // Measured `set -- -ab -b val`: `a` with OPTIND 1 (mid-word), then `b` with
    // OPTARG `-b` and OPTIND 3, and the walk ends on `val` at 3.
    let cell = seeded(&[b"-ab", b"-b", b"val"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 2));
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 3, 0));
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0), "the measured end");
}

#[test]
fn the_terminator_is_consumed() {
    // Measured `set -- -a -- x`: `a` (OPTIND 2), then the end naming the word
    // **after** the terminator: OPTIND 3, status 1.
    let cell = seeded(&[b"-a", b"--", b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 2, 0));
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0));
}

#[test]
fn the_terminator_terminates_on_the_first_word() {
    // Measured `set -- -- x`: no option at all, end status 1, OPTIND 2.
    let cell = seeded(&[b"--", b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 2, 0));
}

#[test]
fn a_terminator_as_an_option_argument_is_no_terminator() {
    // `set -- -b -- x`: `b` takes `--` as its separate option-argument (the
    // scan never consults the terminator rule on a consumed word), landing on
    // 3; the walk then ends on `x` at 3.
    let cell = seeded(&[b"-b", b"--", b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 3, 0));
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0));
}

#[test]
fn a_lone_dash_ends_the_scan_on_its_own_index() {
    // Measured `set -- -`: the end, status 1 — OPTIND stays on the dash's own
    // index, it does not advance to `$#`+1 (2).
    let cell = seeded(&[b"-"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 1, 0));
}

#[test]
fn past_the_end_the_counter_clamps_to_the_list_len_plus_one() {
    // Measured `OPTIND=9` on the list `-a b`: the end answers OPTIND **3**,
    // status 1 — the clamp, not the assigned value.
    let cell = seeded(&[b"-a", b"b"]);
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_var(c"OPTIND".into(), ImportedStr::shell(c"9".into()));
    }
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0));
}

#[test]
fn index_zero_behaves_as_one() {
    // Measured `OPTIND=0` mid-scan: re-read word 1 from its start — `-ab x`
    // reports `a` again and stays on the word.
    let cell = seeded(&[b"-ab", b"x"]);
    {
        let mut state = cell.borrow_mut().unwrap();
        state.set_var(c"OPTIND".into(), ImportedStr::shell(c"0".into()));
    }
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 2));
}

#[test]
fn optind_1_between_two_sets_rescans_the_second_from_word_one() {
    // Measured: set 1 `set -- -abc x` reports `a` mid-word; `OPTIND=1` (the
    // assignment itself, through `set_var`) resets the counter **and** the
    // cursor; set 2 `set -- -b val` then reports `b` with `val` at OPTIND 3.
    let cell = seeded(&[b"-abc", b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 2));
    {
        let mut state = cell.borrow_mut().unwrap();
        let pos: VecDeque<ImportedStr> = [b"-b".to_vec(), b"val".to_vec()]
            .iter()
            .map(|p| ImportedStr::shell(s(p)))
            .collect();
        state.set_positional(pos);
        state.set_var(c"OPTIND".into(), ImportedStr::shell(c"1".into()));
    }
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!(
        (code, index, cursor),
        (0, 3, 0),
        "b takes val from word 1 of set 2"
    );
}

#[test]
fn two_sets_in_a_row_without_a_reset_continue_where_the_counter_stands() {
    // Measured: set 1 `set -- -ab x` runs to its end (the counter sits at
    // `$#`+1 = 3); set 2 answers the end at the clamped index 3, status 1.
    let cell = seeded(&[b"-ab", b"x"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 2), "a");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!(
        (code, index, cursor),
        (0, 3, 0),
        "b takes the whole word `x`"
    );
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0), "set 1's measured end");
    {
        let mut state = cell.borrow_mut().unwrap();
        let pos: VecDeque<ImportedStr> = [b"-b".to_vec(), b"val".to_vec()]
            .iter()
            .map(|p| ImportedStr::shell(s(p)))
            .collect();
        state.set_positional(pos);
    }
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 3, 0), "the measured second end");
}

#[test]
fn processing_continues_after_a_presentation_error() {
    // Measured `set -- -a -x -b val` on `"ab:c"`: `a` (OPTIND 2), the `-x` error
    // (name `?`, rc 0, OPTIND 3), `b`=`val` (OPTIND 5) — bash's rule, the
    // one-line decision of `scan.rs` (POSIX stops at the error; the statuses of
    // the errors themselves are task #591's — bash measures 0 here).
    let cell = seeded(&[b"-a", b"-x", b"-b", b"val"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 2, 0), "a");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 3, 0), "the -x error continues");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 5, 0), "b takes val");
}

#[test]
fn a_missing_option_argument_reports_and_continues_to_the_end() {
    // Measured `set -- -b`: the error path answers name `?` (verbose) with rc 0
    // and OPTIND 2 — the scan data behind it is pinned in `scan/tests.rs`; the
    // end of the one-word list then lands on 3 = `$#`+1 of the two-word list.
    let cell = seeded(&[b"-b"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!(
        (code, index, cursor),
        (0, 2, 0),
        "the measured error status"
    );
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 2, 0), "past the end: $# + 1");
}

#[test]
fn the_error_walk_on_one_word_reports_per_character() {
    // Measured `set -- -cval`: `c` (OPTIND 1), the `v` error (OPTIND 1), `a`
    // (OPTIND 1), the `l` error (OPTIND 2), then the end at 2 — asserted through
    // the committed counter and cursor (bash's statuses are 0 for every report,
    // 1 for the end).
    let cell = seeded(&[b"-cval"]);
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 2), "c");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 3), "the v error");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (0, 1, 4), "a");
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!(
        (code, index, cursor),
        (0, 2, 0),
        "the l error ends the word"
    );
    let (code, index, cursor) = run(&cell, &[b"ab:c", b"o"]);
    assert_eq!((code, index, cursor), (1, 2, 0), "the measured end");
}

// ---------------------------------------------------------------------------
// The per-call writes of task #591: `name`, `OPTIND`, `OPTARG`, the statuses and
// the diagnostics. Every row re-measured on bash 5.3.9 on this machine before
// being pinned (the plan §2 matrix).

/// `(name, OPTARG, OPTIND, cursor, status)` after one call, read back the way the
/// caller reads it — the store, `$name` and `$?`. `None` is a variable the call
/// never wrote, which for `OPTARG` is bash's **unbound** (its `${OPTARG+set}`
/// answers empty, plan §2), not an empty value.
fn called(
    cell: &ForkCell<ShellState>,
    args: &[&[u8]],
    name: &[u8],
) -> (Option<Vec<u8>>, Option<Vec<u8>>, u64, usize, i32) {
    let cmdline = cmdline_with(args);
    assert!(
        run_getopts(b"getopts o", &cmdline, cell).unwrap(),
        "getopts is claimed (the launch path never sees it)"
    );
    let state = cell.borrow().unwrap();
    let name = s(name);
    let optarg = ShortCStr::from(c"OPTARG");
    (
        state.strings.get(&name).map(store_bytes),
        state.strings.get(&optarg).map(store_bytes),
        state.optind,
        state.optchar,
        state.last_status.exit_code(),
    )
}

fn store_bytes(value: &ImportedStr) -> Vec<u8> {
    value.value.as_bytes().unwrap().to_vec()
}

#[test]
fn a_found_flag_names_its_character_and_leaves_optarg_unbound() {
    // Measured `set -- -a` on `"ab:c"`: name `a`, OPTIND 2, OPTARG unbound, rc 0.
    let cell = seeded(&[b"-a"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"a".to_vec()), None, 2, 0, 0)
    );
}

#[test]
fn an_attached_argument_is_the_arguments_row() {
    // Measured `set -- -bval`: name `b`, OPTARG `val`, OPTIND 2, rc 0.
    let cell = seeded(&[b"-bval"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"b".to_vec()), Some(b"val".to_vec()), 2, 0, 0)
    );
}

#[test]
fn a_separate_argument_is_the_arguments_row_too() {
    // Measured `set -- -b val`: name `b`, OPTARG `val`, OPTIND 3 — the word the
    // argument consumed moved the counter too (plan §2).
    let cell = seeded(&[b"-b", b"val"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"b".to_vec()), Some(b"val".to_vec()), 3, 0, 0)
    );
}

#[test]
fn a_flag_unties_the_optarg_a_previous_call_set() {
    // Measured on `":ab:c"` (silent): `set -- -x -a` — call 1 is the unknown-option
    // row (name `?`, OPTARG the offender `x`, rc 0, nothing printed); call 2 reports
    // the flag and `OPTARG` goes back to **unbound** (plan §2's fourth row).
    let cell = seeded(&[b"-x", b"-a"]);
    assert_eq!(
        called(&cell, &[b":ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), Some(b"x".to_vec()), 2, 0, 0)
    );
    assert_eq!(
        called(&cell, &[b":ab:c", b"o"], b"o"),
        (Some(b"a".to_vec()), None, 3, 0, 0)
    );
}

#[test]
fn the_end_names_the_question_mark_with_optarg_unbound() {
    // Measured `set -- foo`: the walk ends on the first non-option word — name `?`,
    // OPTIND 1, OPTARG unbound, rc 1, nothing printed (plan §2).
    let cell = seeded(&[b"foo"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 1, 0, 1)
    );
}

#[test]
fn the_end_after_a_found_argument_keeps_the_counter_it_committed() {
    // Measured `set -- -b val x`, calls 1-2-3: `b` with `val` at OPTIND 3 (rc 0),
    // then the end twice: name `?`, OPTARG unbound, rc 1, OPTIND stays 3.
    let cell = seeded(&[b"-b", b"val", b"x"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"b".to_vec()), Some(b"val".to_vec()), 3, 0, 0)
    );
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 3, 0, 1)
    );
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 3, 0, 1),
        "the second end, bash's rc 1 again"
    );
}

#[test]
fn an_unknown_option_names_the_question_mark_and_prints() {
    // Measured `set -- -x` on `"ab:c"` (verbose): name `?`, OPTARG unbound, OPTIND 2
    // (the offending word is passed), rc 0 — and the message is the builder's, so
    // `diagnostic::presentation` answers Some for this row (its bytes are pinned in
    // `diagnostic/tests.rs`).
    let cell = seeded(&[b"-x"]);
    assert_eq!(
        // The printed bytes are pinned in `diagnostic/tests.rs`; this is the
        // store half of the row.
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 2, 0, 0)
    );
}

#[test]
fn the_silent_mode_answers_optarg_with_the_offender() {
    // Measured `set -- -x` on `":ab:c"`: name `?`, OPTARG `x`, OPTIND 2, rc 0, and
    // **no** diagnostic (plan §2, POSIX agrees).
    let cell = seeded(&[b"-x"]);
    assert_eq!(
        called(&cell, &[b":ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), Some(b"x".to_vec()), 2, 0, 0)
    );
}

#[test]
fn a_missing_option_argument_is_the_option_character_in_the_verbose_mode() {
    // Measured `set -- -b` on `"ab:c"`: name `?`, OPTARG unbound, OPTIND 2, rc 0, and
    // bash **does** print `option requires an argument -- b` here (re-measured on
    // task591: bash 5.3.9 answers POSIX's text on this row).
    let cell = seeded(&[b"-b"]);
    assert_eq!(
        // The printed bytes are pinned in `diagnostic/tests.rs`; this is the
        // store half of the row.
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"?".to_vec()), None, 2, 0, 0)
    );
}

#[test]
fn a_missing_option_argument_is_a_colon_in_the_silent_mode() {
    // Measured `set -- -b` on `":ab:c"`: name `:`, OPTARG `b`, OPTIND 2, rc 0, nothing
    // printed — the one row whose name is not `?` (plan §2/§4.1).
    let cell = seeded(&[b"-b"]);
    assert_eq!(
        called(&cell, &[b":ab:c", b"o"], b"o"),
        (Some(b":".to_vec()), Some(b"b".to_vec()), 2, 0, 0)
    );
}

#[test]
fn a_name_that_is_not_an_identifier_fails_only_the_name_write() {
    // Measured `set -- -bval; getopts "ab:c" 1abc`: the scan runs and **commits** —
    // OPTIND 2 and OPTARG `val` stand — only the name write fails, with bash's
    // message and status 1 (plan §2, §3.1).
    let cell = seeded(&[b"-bval"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"1abc"], b"1abc"),
        (None, Some(b"val".to_vec()), 2, 0, 1)
    );
}

#[test]
fn an_empty_name_operand_is_the_same_error_on_a_found_option() {
    // Measured `set -- -a; getopts "ab:c" 1abc` (the first row of the plan §2
    // processing-error table): the counter commits, nothing is named, rc 1. The
    // **empty** operand (`getopts "ab:c" ""`) is the same rule with nothing else to
    // commit: an empty word list, so the end row's OPTIND 1 and rc 1.
    let cell = seeded(&[b"-a"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"1abc"], b"1abc"),
        (None, None, 2, 0, 1)
    );
    let cell = ForkCell::new(ShellState::new());
    assert_eq!(
        called(&cell, &[b"ab:c", b""], b""),
        (None, None, 1, 0, 1),
        "the empty operand, nothing else to commit"
    );
}

#[test]
fn an_accepted_name_operand_is_written() {
    // Measured `getopts "ab:c" v -a`: `v` is an identifier, so the row's character
    // lands in it (`a`), `OPTARG` stays unbound, OPTIND 2, rc 0 — no message (the
    // accept half of the identifier rule; `vars/tests.rs` pins the whole matrix).
    let cell = ForkCell::new(ShellState::new());
    assert_eq!(
        called(&cell, &[b"ab:c", b"v", b"-a"], b"v"),
        (Some(b"a".to_vec()), None, 2, 0, 0)
    );
}

#[test]
fn a_missing_name_operand_answers_the_usage_line_and_keeps_running() {
    // Measured `getopts "ab:c"`: bash prints the usage line, answers **2**, writes no
    // variable and runs no scan (OPTIND stays 1) — a status, not a `Report`, so the
    // script continues (plan §3.1/§4.4).
    let cell = seeded(&[b"-a"]);
    cell.borrow_mut().unwrap().set_last_exit(0);
    assert_eq!(
        called(&cell, &[b"ab:c"], b"o"),
        (None, None, 1, 0, 2),
        "the usage row writes nothing"
    );
}

#[test]
fn a_missing_optstring_operand_answers_the_usage_line_too() {
    // Measured `getopts`: the same usage line, the same status 2, nothing set.
    let cell = seeded(&[b"-a"]);
    cell.borrow_mut().unwrap().set_last_exit(0);
    let cmdline = cmdline_with(&[]);
    assert!(run_getopts(b"getopts", &cmdline, &cell).unwrap());
    let state = cell.borrow().unwrap();
    assert_eq!(state.last_status.exit_code(), 2, "bash's measured 2");
    assert_eq!(state.optind, DEFAULT_OPTIND, "no scan ran");
    assert!(
        state.strings.get(&ShortCStr::from(c"OPTARG")).is_none(),
        "nothing written"
    );
}

#[test]
fn the_counter_write_moves_the_visible_row_and_leaves_the_cursor_alone() {
    // Plan §3.4: the counter's commit writes the counter **and** the store row
    // bash's `$OPTIND` reads, and must not go through the `set_var` name-sync — a
    // cluster whose counter has not moved yet (`set -- -abc val`: `a` reports, the
    // counter stays 1, the cursor moves to 2) proves both halves at once.
    let cell = seeded(&[b"-abc", b"val"]);
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"a".to_vec()), None, 1, 2, 0),
        "the row agrees with the field, the cursor moved"
    );
    let state = cell.borrow().unwrap();
    let row = state
        .strings
        .get(&ShortCStr::from(c"OPTIND"))
        .map(store_bytes)
        .unwrap();
    assert_eq!(row, b"1", "the visible row of the counter's own value");
    drop(state);
    // And the second call, resuming the cluster: `b` takes the attached `c`, the
    // counter moves to 2, and the row moves with it.
    assert_eq!(
        called(&cell, &[b"ab:c", b"o"], b"o"),
        (Some(b"b".to_vec()), Some(b"c".to_vec()), 2, 0, 0)
    );
    let state = cell.borrow().unwrap();
    let row = state
        .strings
        .get(&ShortCStr::from(c"OPTIND"))
        .map(store_bytes)
        .unwrap();
    assert_eq!(row, b"2", "the row followed the counter");
}

/// The one write-observing pin of plan §6: the driver's "message present → write it"
/// branch, seen as the **bytes in the file** the call redirects stderr to. The
/// in-process `$0` of `ShellState::new()` is `fdshell` (the seed `state/ctor.rs`
/// pins), and the row is the verbose unknown-option message.
#[test]
fn the_verbose_diagnostic_reaches_the_redirected_stderr() {
    let dir = scratch("write");
    let path = dir.0.join("err");
    let cell = seeded(&[b"-x"]);
    let line = format!(r#"getopts "ab:c" o 2>{}"#, path.display());
    let text = ScriptText::new(s(line.as_bytes()), Position::new(1, 1), Origin::Shell);
    crate::run::run_one(&text, &cell, false).unwrap();
    assert_eq!(
        std::fs::read(path).unwrap(),
        b"fdshell: illegal option -- x\n"
    );
}

/// A pid + counter scratch dir, removed on drop (the `intercept/cd/tests.rs` shape,
/// so a concurrent suite run cannot collide).
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn scratch(tag: &str) -> Scratch {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "fdshell-getopts-{}-{}-{}",
        tag,
        std::process::id(),
        c
    ));
    std::fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}
