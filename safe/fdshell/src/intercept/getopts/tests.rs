#![allow(clippy::unwrap_used)]
use super::*;
use crate::state::ShellState;
use alloc::collections::VecDeque;
use alloc::vec;
use sys::ImportedStr;
use sys::fork_cell::ForkCell;

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
/// list). bash answers `name='?'` there; the variable write is task #591's, so
/// this test asserts the status and the counters, not stderr.
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
