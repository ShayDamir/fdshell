use std::collections::HashMap;
use std::collections::VecDeque;

use sys::LocalFd;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

use crate::envfilter::EnvFilter;
use crate::task::Task;

pub struct ShellState {
    pub fds: HashMap<ShortCStr, LocalFd>,
    pub tasks: HashMap<ShortCStr, Task>,
    pub strings: HashMap<ShortCStr, ShortCStr>,
    pub exports: HashMap<ShortCStr, Vec<u8>>,
    pub positional: VecDeque<ShortCStr>,
    pub last_status: WaitStatus,
    pub shell_pid: i32,
    pub last_bg_pid: Option<i32>,
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
}
