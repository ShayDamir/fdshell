use alloc::collections::VecDeque;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ImportedStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

use crate::envfilter::EnvFilter;
use frames::VarFrame;
use jobs::Jobs;

pub use fds::{FdArrayEntry, FdVar};

pub struct ShellState {
    pub(crate) fds: HashMap<ShortCStr, FdVar>,
    pub(crate) arrays: HashMap<ShortCStr, Vec<FdArrayEntry>>,
    /// The background jobs: named (pidvar) + nameless (plain `&`) — see [`Jobs`].
    pub(crate) jobs: Jobs,
    pub(crate) strings: HashMap<ShortCStr, ImportedStr>,
    pub(crate) exports: HashMap<ShortCStr, ImportedStr>,
    /// `$1..$N` only; 0-based storage (`get(k)` is `$(k + 1)`).
    pub(crate) positional: VecDeque<ImportedStr>,
    /// `$0`: the program name, never a slot of `positional` (bash's argv[0]).
    pub(crate) program_name: ImportedStr,
    /// One frame per active function call: the `local` shadows to restore on return.
    pub(crate) frames: Vec<VarFrame>,
    pub(crate) last_status: WaitStatus,
    pub(crate) shell_pid: sys::Pid,
    pub(crate) last_bg_pid: Option<sys::Pid>,
    /// Reaped-children CPU times (microseconds), accumulated at each reap for
    /// `times` (Linux's `getrusage` carries no children fields).
    pub(crate) child_utime: u64,
    pub(crate) child_stime: u64,
    pub(crate) env_filter: EnvFilter,
    pub(crate) shell_sock: Option<LocalFd>,
    pub(crate) environ: Vec<(ShortCStr, ShortCStr)>,
    pub(crate) nesting: u32,
    /// Number of `for`/`while`/`until` loops enclosing the command being executed
    /// (`break n` / `continue n` counts against it; `if`, `case` and groups are transparent).
    pub(crate) loop_depth: u32,
    pub(crate) eval_depth: u32,
    pub(crate) ifs: ShortCStr,
    pub(crate) options: u32,
    pub(crate) capture_limit: usize,
    pub(crate) hash_table: HashMap<ShortCStr, ShortCStr>,
    pub(crate) aliases: HashMap<ShortCStr, ShortCStr>,
    pub(crate) functions: HashMap<ShortCStr, ShortCStr>,
}

impl ShellState {
    /// The default `IFS`: space, tab, newline (POSIX's default split set). One
    /// owner — the seed of [`ShellState::new`] and the value `unset IFS`
    /// restores (bash, task #290).
    pub(crate) fn default_ifs() -> ShortCStr {
        c" \t\n".into()
    }

    pub fn new() -> Self {
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
            eval_depth: 0,
            ifs: Self::default_ifs(),
            options: crate::options::DEFAULTS,
            capture_limit: crate::cmd_subst::MAX_CAPTURED,
            hash_table: HashMap::new(),
            aliases: HashMap::new(),
            functions: HashMap::new(),
        }
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

mod arrays;
mod cwd;
pub(crate) mod fds;
pub(crate) mod frames;
pub(crate) mod jobs;
mod options;
mod setters;

#[cfg(test)]
mod tests;
