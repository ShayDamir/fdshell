use alloc::collections::VecDeque;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ImportedStr;
use sys::LocalFd;
use sys::ShortCStr;
use sys::Trace;
use sys::siginfo::WaitStatus;

use crate::envfilter::EnvFilter;
use crate::task::Task;

/// An fd variable: an owned descriptor together with its provenance trace.
pub struct FdVar {
    pub fd: LocalFd,
    pub trace: Trace,
}

pub struct ShellState {
    pub(crate) fds: HashMap<ShortCStr, FdVar>,
    pub(crate) tasks: HashMap<ShortCStr, Task>,
    pub(crate) strings: HashMap<ShortCStr, ImportedStr>,
    pub(crate) exports: HashMap<ShortCStr, ImportedStr>,
    pub(crate) positional: VecDeque<ImportedStr>,
    pub(crate) last_status: WaitStatus,
    pub(crate) shell_pid: sys::Pid,
    pub(crate) last_bg_pid: Option<sys::Pid>,
    pub(crate) env_filter: EnvFilter,
    pub(crate) shell_sock: Option<LocalFd>,
    pub(crate) environ: Vec<(ShortCStr, ShortCStr)>,
    pub(crate) nesting: u32,
    pub(crate) eval_depth: u32,
    pub(crate) ifs: ShortCStr,
    pub(crate) options: u32,
    pub(crate) aliases: HashMap<ShortCStr, ShortCStr>,
    pub(crate) functions: HashMap<ShortCStr, ShortCStr>,
}

impl ShellState {
    pub fn new() -> Self {
        ShellState {
            fds: HashMap::new(),
            tasks: HashMap::new(),
            strings: HashMap::new(),
            exports: HashMap::new(),
            positional: VecDeque::new(),
            last_status: WaitStatus::Exited(0),
            shell_pid: sys::env::getpid(),
            last_bg_pid: None,
            env_filter: EnvFilter::new(),
            shell_sock: None,
            environ: sys::env::environ_snapshot(),
            nesting: 0,
            eval_depth: 0,
            ifs: c" \t\n".into(),
            options: crate::options::DEFAULTS,
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

mod setters;

#[cfg(test)]
mod tests;
