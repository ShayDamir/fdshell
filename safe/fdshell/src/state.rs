use alloc::collections::VecDeque;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ImportedStr;
use sys::LocalFd;
use sys::Origin;
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
    pub(crate) ifs: ShortCStr,
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
            ifs: c" \t\n".into(),
        }
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellState {
    pub fn shift(&mut self, n: usize) {
        for _ in 0..n.min(self.positional.len()) {
            self.positional.pop_front();
        }
    }

    pub fn insert_cwd(&mut self, cwd: LocalFd) {
        self.fds.insert(
            c"CWD".into(),
            FdVar {
                fd: cwd,
                trace: Trace::boundary(Origin::Shell),
            },
        );
    }

    pub fn set_positional(&mut self, positional: VecDeque<ImportedStr>) {
        self.positional = positional;
    }

    pub fn set_last_exit(&mut self, code: i32) {
        self.last_status = WaitStatus::Exited(code);
    }

    pub fn set_shell_sock(&mut self, sock: LocalFd) {
        self.shell_sock = Some(sock);
    }

    pub fn set_ifs(&mut self, ifs: ShortCStr) {
        self.ifs = ifs;
    }
}

#[cfg(test)]
mod tests;
