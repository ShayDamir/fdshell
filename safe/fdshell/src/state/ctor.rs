//! The shell state's **construction**: the field-by-field seed of
//! [`ShellState::new`] (task #439). The struct and its default `IFS` stay with
//! [`super`]; this file is the single place that spells out what a fresh shell
//! starts with — a `HashMap` of nothing, the `environ` snapshot, the program
//! name `fdshell`, the default option words, the command-substitution capture
//! limit. `ShellState::new` and `Default` both land here, so the seed is one
//! list and has one owner.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

use super::{ShellState, jobs::Jobs};
use crate::envfilter::EnvFilter;

/// The seed of a fresh shell: every field of [`ShellState`] spelled out once.
///
/// The two values that are not literals are read from the process, not from
/// this file: `shell_pid` (`getpid`) and `environ` (the environment snapshot).
/// `ifs` is [`ShellState::default_ifs`], the same value `unset IFS` restores.
pub(crate) fn build() -> ShellState {
    ShellState {
        fds: HashMap::new(),
        arrays: HashMap::new(),
        jobs: Jobs::new(),
        strings: HashMap::new(),
        exports: HashMap::new(),
        positional: VecDeque::new(),
        program_name: ImportedStr::shell(ShortCStr::from(c"fdshell")),
        frames: Vec::new(),
        last_status: WaitStatus::Exited(0),
        shell_pid: sys::env::getpid(),
        last_bg_pid: None,
        child_utime: 0,
        child_stime: 0,
        env_filter: EnvFilter::new(),
        shell_sock: None,
        environ: sys::env::environ_snapshot(),
        nesting: 0,
        loop_depth: 0,
        errexit_gated: false,
        eval_depth: 0,
        ifs: ShellState::default_ifs(),
        options: crate::options::DEFAULTS,
        interactive: false,
        capture_limit: crate::cmd_subst::MAX_CAPTURED,
        hash_table: HashMap::new(),
        aliases: HashMap::new(),
        functions: HashMap::new(),
    }
}
