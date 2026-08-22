use alloc::collections::VecDeque;

use sys::ImportedStr;
use sys::LocalFd;
use sys::Origin;
use sys::ShortCStr;
use sys::Trace;
use sys::siginfo::WaitStatus;

use super::{FdVar, ShellState};

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

    /// Store a shell string variable, keeping `ifs` in sync when the name is `IFS`.
    pub fn set_var(&mut self, name: ShortCStr, value: ImportedStr) {
        if name.eq_bytes(b"IFS") {
            self.ifs = value.value.clone();
        }
        self.strings.insert(name, value);
    }
}
