#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;
use core::ffi::CStr;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::state::ShellState;
use crate::task::Task;
use sys::fork_cell::ForkCell;
use sys::getpwuid::getpwuid;
use sys::unistd::{geteuid, gethostname};
use sys::{ImportedStr, ShortCStr};

use super::{ShellValues, basename, cut_at_first_dot};

/// A state whose `interactive` flag is what `seed_state` seeds from the
/// `is_tty` syscall, so the resolver's tty gate is pinnable in-process (LESSONS:
/// caching a syscall's answer moves the branch into data).
fn cell(interactive: bool) -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    cell.borrow_mut().unwrap().interactive = interactive;
    cell
}

fn provider(cell: &ForkCell<ShellState>) -> ShellValues<'_> {
    ShellValues::new(cell)
}

fn resolve(cell: &ForkCell<ShellState>, name: &'static CStr, default: &[u8]) -> Vec<u8> {
    crate::repl::prompt::resolve(cell, &ShortCStr::from(name), default).unwrap()
}

/// A live child (exits 0) as a `Task` — the job tables' entries own a real pidfd,
/// so a test that seeds one forks it and reaps it itself (hermetic: no child
/// outlives the test, LESSONS).
fn child_task() -> Task {
    let (pid, pidfd_opt) = sys::fork_pidfd::fork_pidfd().unwrap();
    match pidfd_opt {
        None => sys::exit(0),
        Some(pidfd) => Task {
            pidfd,
            capture_fd: None,
            child_pid: pid,
            captures: Vec::new(),
        },
    }
}

fn reap(tasks: impl IntoIterator<Item = Task>) {
    for task in tasks {
        let _ = task.pidfd.wait_pidfd_rusage();
    }
}

#[test]
fn u_answers_the_same_lookup_the_wrapper_does() {
    // Asserted against the wrapper, not a literal: a box whose password database
    // has no entry for the runner answers `None`, and that is the fallback the
    // renderer draws literally (decision D2).
    let cell = cell(true);
    assert_eq!(
        provider(&cell).value(ValueEscape::Username),
        getpwuid(geteuid())
    );
}

#[test]
fn h_and_full_hostname_answer_the_same_gethostname_call() {
    let cell = cell(true);
    let host = gethostname().unwrap();
    assert_eq!(
        provider(&cell).value(ValueEscape::Hostname),
        Some(cut_at_first_dot(&host))
    );
    assert_eq!(provider(&cell).value(ValueEscape::FullHostname), Some(host));
}

#[test]
fn the_dot_rule_stops_at_the_first_dot() {
    // The rule, hard-coded — this box's hostname has no dot, so the pin above
    // cannot see the split (bash's `\h` and `\H` agree on a dotless name).
    assert_eq!(cut_at_first_dot(b"alpha.beta.gamma"), b"alpha");
    assert_eq!(cut_at_first_dot(b"alpha."), b"alpha");
    assert_eq!(cut_at_first_dot(b"alpha"), b"alpha");
}

#[test]
fn the_basename_rule_is_the_last_slash_component() {
    // The rule, hard-coded. bash draws the same on a terminal (`[3] ` for
    // `/dev/pts/3`, measured 2026-11-18); the pty row in
    // `safe/fdshell/tests/repl.rs` pins the terminal case end to end.
    assert_eq!(basename(b"/dev/pts/3"), b"3");
    assert_eq!(basename(b"pts/3"), b"3");
    assert_eq!(basename(b"3"), b"3");
}

#[test]
fn dollar_answers_for_the_runners_effective_uid() {
    // The branch that matches the runner (nextest runs non-root); bash's own
    // prompt prints `#` only for the superuser.
    let cell = cell(true);
    let want = if geteuid() == 0 { b"#" } else { b"$" };
    assert_eq!(
        provider(&cell).value(ValueEscape::Dollars),
        Some(want.to_vec())
    );
}

#[test]
fn a_pipe_stdin_has_no_tty_name_so_the_escape_stays_literal() {
    // nextest's stdin is a pipe: `ttyname` fails, the provider answers `None`, and
    // D2's fallback draws the escape text itself. The terminal case is pinned by
    // the pty rows in `safe/fdshell/tests/repl.rs`; bash's no-terminal answer is
    // `tty` (measured), which is that provider's own business.
    let cell = cell(true);
    assert_eq!(provider(&cell).value(ValueEscape::TtyName), None);
    let ps1 = ShortCStr::from(c"PS1");
    let value = ImportedStr::shell(ShortCStr::from(c"a\\lb"));
    cell.borrow_mut().unwrap().set_var(ps1.clone(), value);
    assert_eq!(resolve(&cell, c"PS1", b"ignored"), br"a\lb");
}

#[test]
fn s_follows_the_program_name_and_its_setter() {
    let cell = cell(true);
    assert_eq!(
        provider(&cell).value(ValueEscape::ShellName),
        Some(b"fdshell".to_vec())
    );
    cell.borrow_mut()
        .unwrap()
        .set_program_name(ImportedStr::shell(ShortCStr::from(c"marker")));
    assert_eq!(
        provider(&cell).value(ValueEscape::ShellName),
        Some(b"marker".to_vec())
    );
}

#[test]
fn v_and_full_version_draw_the_crates_own_version() {
    // fdshell has no patchlevel concept, so `\V` is `\v` (the epic's minimal
    // definition). bash draws its own identity here (`bash`, `5.3`, `5.3.9`) —
    // the divergence #623 documents.
    let cell = cell(true);
    let want = env!("CARGO_PKG_VERSION").as_bytes().to_vec();
    assert_eq!(
        provider(&cell).value(ValueEscape::Version),
        Some(want.clone())
    );
    assert_eq!(provider(&cell).value(ValueEscape::PlusVersion), Some(want));
}

#[test]
fn j_is_the_count_of_both_task_maps() {
    let cell = cell(true);
    assert_eq!(
        provider(&cell).value(ValueEscape::Jobs),
        Some(b"0".to_vec())
    );
    let (named, anon) = (child_task(), child_task());
    {
        let mut state = cell.borrow_mut().unwrap();
        state.jobs.insert_named(ShortCStr::from(c"named"), named);
        state.jobs.push_anon(anon);
    }
    assert_eq!(
        provider(&cell).value(ValueEscape::Jobs),
        Some(b"2".to_vec())
    );
    let mut state = cell.borrow_mut().unwrap();
    reap(state.jobs.drain_named());
    reap(state.jobs.drain_anon());
}

#[test]
fn the_counters_draw_the_number_the_command_gets() {
    // Fresh state: the command about to be typed is number 1 (bash 5.3.9 measured:
    // `[263|1]` at the first prompt of a session whose history file is not empty —
    // `\!` counts that list, and fdshell has none yet, story #21, so both escapes
    // are this one counter).
    let cell = cell(true);
    assert_eq!(
        provider(&cell).value(ValueEscape::History),
        Some(b"1".to_vec())
    );
    assert_eq!(
        provider(&cell).value(ValueEscape::AlignedNumber),
        Some(b"1".to_vec())
    );
    cell.borrow_mut().unwrap().command_number = 5;
    assert_eq!(
        provider(&cell).value(ValueEscape::History),
        Some(b"6".to_vec())
    );
    assert_eq!(
        provider(&cell).value(ValueEscape::AlignedNumber),
        Some(b"6".to_vec())
    );
}

#[test]
fn the_sibling_owners_escapes_answer_no_value() {
    // One provider, one family: this provider answers `None` for all seven.
    // #618's clock escapes are answered now — by `clock.rs`'s own provider
    // through the composition — and only #619's remain to render literally.
    let cell = cell(true);
    for esc in [
        ValueEscape::Date,
        ValueEscape::Time,
        ValueEscape::Time24,
        ValueEscape::AmPmTime,
        ValueEscape::MinuteTime,
        ValueEscape::WorkingDir,
        ValueEscape::ShortWorkingDir,
    ] {
        assert_eq!(provider(&cell).value(esc), None, "{esc:?} is a sibling's");
    }
}
