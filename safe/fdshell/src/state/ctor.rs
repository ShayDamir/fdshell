//! The shell state's **construction**: the field-by-field seed of
//! [`ShellState::new`] (task #439). The struct and its default `IFS` stay with
//! [`super`]; this file is the single place that spells out what a fresh shell
//! starts with — the `IFS` entry of the string store, the `environ` snapshot
//! with its `IFS` value rewritten, a `HashMap` of nothing, the program name
//! `fdshell`, the default option words, the command-substitution capture limit.
//! `ShellState::new` and `Default` both land here, so the seed is one list and
//! has one owner.

// mutants: -C --lib -- -E 'test(/^state::/)'

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

use super::{ShellState, jobs::Jobs};
use crate::envfilter::EnvFilter;

/// The seed of the string store: `IFS`, set to the default split set, and
/// `OPTIND`, set to the counter's default (task #589). POSIX XCU
/// 2.5.1 (bash 5.3.9, measured) has the shell set `IFS` to its default value at
/// startup **whether or not the parent exported it**, so the store — not the
/// `environ` snapshot — owns the value `$IFS` reports, and the store's absence
/// keeps `unset IFS`'s meaning: the name is unbound afterwards. bash 5.3.9 measures
/// the same visible seed for `OPTIND` (`set` answers `OPTIND=1`), and the store is
/// again the only owner: the seed is not exported, so no child sees the row.
pub(super) fn seeded_strings() -> HashMap<ShortCStr, ImportedStr> {
    let mut strings = HashMap::new();
    let ifs: ShortCStr = c"IFS".into();
    strings.insert(ifs, ImportedStr::shell(ShellState::default_ifs()));
    let optind: ShortCStr = c"OPTIND".into();
    strings.insert(optind, ImportedStr::shell(ShortCStr::from(c"1")));
    strings
}

/// The `environ` snapshot with only one value rewritten: an inherited `IFS` row
/// carries the **default** split set, not the parent's bytes. The row itself
/// stays — for the child environment it is fdshell's carrier of "the parent
/// exported this name", and bash inherits exactly that attribute while resetting
/// the value. Every other row is the parent's, untouched.
pub(super) fn seeded_environ(
    inherited: Vec<(ShortCStr, ShortCStr)>,
) -> Vec<(ShortCStr, ShortCStr)> {
    inherited
        .into_iter()
        .map(|(name, value)| {
            if name.eq_bytes(b"IFS") {
                (name, ShellState::default_ifs())
            } else {
                (name, value)
            }
        })
        .collect()
}

/// The seed of [`ShellState::ignored_on_entry`]: one `sigaction` query per signal
/// in `1..=64`, bit `sig - 1` set for the ones the process already ignores (the
/// same bit shape `sys`'s pending mask uses). A query that fails answers
/// not-ignored, so the seed of a fresh shell never fails; in practice exactly one
/// bit is set here — Rust's own `SIG_IGN` for `SIGPIPE`.
pub(super) fn seeded_ignored_on_entry() -> u64 {
    (1..=64).fold(0u64, |mask, sig| {
        let ignored = sys::signal::disposition(sig).unwrap_or(0) == sys::signal::SIG_IGN;
        if ignored {
            mask | (1u64 << (sig - 1))
        } else {
            mask
        }
    })
}

/// The seed of a fresh shell: every field of [`ShellState`] spelled out once.
///
/// The three values that are not literals are read from the process, not from
/// this file: `shell_pid` (`getpid`), `environ` (the environment snapshot,
/// rewritten by [`seeded_environ`]) and [`seeded_ignored_on_entry`] (a
/// `sigaction` query per signal). The three `IFS` seeds — `ifs`,
/// [`seeded_strings`] and the rewrite — are all [`ShellState::default_ifs`], the
/// same value `unset IFS` restores. The two `OPTIND` seeds — the store row and the
/// field — are both [`crate::intercept::getopts::DEFAULT_OPTIND`], the value
/// `unset OPTIND` restores (task #589).
pub(crate) fn build() -> ShellState {
    ShellState {
        fds: HashMap::new(),
        arrays: HashMap::new(),
        str_arrays: HashMap::new(),
        jobs: Jobs::new(),
        strings: seeded_strings(),
        exports: HashMap::new(),
        positional: VecDeque::new(),
        program_name: ImportedStr::shell(ShortCStr::from(c"fdshell")),
        frames: Vec::new(),
        last_status: WaitStatus::Exited(0),
        word0_vanished: false,
        last_substitution: None,
        shell_pid: sys::env::getpid(),
        last_bg_pid: None,
        child_utime: 0,
        child_stime: 0,
        env_filter: EnvFilter::new(),
        shell_sock: None,
        environ: seeded_environ(sys::env::environ_snapshot()),
        nesting: 0,
        loop_depth: 0,
        errexit_gated: false,
        eval_depth: 0,
        ifs: ShellState::default_ifs(),
        optind: crate::intercept::getopts::DEFAULT_OPTIND,
        optchar: 0,
        options: crate::options::DEFAULTS,
        interactive: false,
        capture_limit: crate::cmd_subst::MAX_CAPTURED,
        hash_table: HashMap::new(),
        aliases: HashMap::new(),
        functions: HashMap::new(),
        traps: BTreeMap::new(),
        ignored_on_entry: seeded_ignored_on_entry(),
    }
}
