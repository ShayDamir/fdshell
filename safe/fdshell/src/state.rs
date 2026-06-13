#![forbid(unsafe_code)]

use crate::task::Task;
use std::collections::HashMap;
use sys::LocalFd;
use sys::ShortCStr;
use sys::siginfo::WaitStatus;

pub struct ShellState {
    pub fds: HashMap<ShortCStr, LocalFd>,
    pub tasks: HashMap<ShortCStr, Task>,
    pub strings: HashMap<ShortCStr, ShortCStr>,
    pub last_status: WaitStatus,
    pub shell_pid: i32,
    pub last_bg_pid: Option<i32>,
}

impl ShellState {
    pub fn new() -> Self {
        ShellState {
            fds: HashMap::new(),
            tasks: HashMap::new(),
            strings: HashMap::new(),
            last_status: WaitStatus::Exited(0),
            shell_pid: std::process::id() as i32,
            last_bg_pid: None,
        }
    }
}
