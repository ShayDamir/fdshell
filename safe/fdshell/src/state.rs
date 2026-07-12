use std::collections::HashMap;
use std::collections::VecDeque;

use sys::LocalFd;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

use crate::envfilter::EnvFilter;
use crate::task::Task;

pub struct ShellState {
    pub(crate) fds: HashMap<ShortCStr, LocalFd>,
    pub(crate) tasks: HashMap<ShortCStr, Task>,
    pub(crate) strings: HashMap<ShortCStr, ShortCStr>,
    pub(crate) exports: HashMap<ShortCStr, Vec<u8>>,
    pub(crate) positional: VecDeque<ShortCStr>,
    pub(crate) last_status: WaitStatus,
    pub(crate) shell_pid: i32,
    pub(crate) last_bg_pid: Option<i32>,
    pub(crate) env_filter: EnvFilter,
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
            shell_pid: std::process::id() as i32,
            last_bg_pid: None,
            env_filter: EnvFilter::new(),
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
        self.fds.insert(c"CWD".into(), cwd);
    }

    pub fn set_positional(&mut self, positional: VecDeque<ShortCStr>) {
        self.positional = positional;
    }

    pub fn set_last_exit(&mut self, code: i32) {
        self.last_status = WaitStatus::Exited(code);
    }
}
