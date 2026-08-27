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

    /// Store the last argument of the previous command as the `_` variable
    /// (bash `$_`), skipping the update while an `eval`/`source` frame runs.
    pub fn set_last_arg(&mut self, arg: ShortCStr) {
        if self.eval_depth == 0 {
            let trace = Trace::boundary(Origin::Shell);
            self.set_var(c"_".into(), ImportedStr::new(arg, trace));
        }
    }

    /// Clear `_` to empty, as bash does after assignments and `for` loops,
    /// skipping the update while an `eval`/`source` frame runs.
    pub fn clear_last_arg(&mut self) {
        if self.eval_depth == 0 {
            let trace = Trace::boundary(Origin::Shell);
            self.set_var(c"_".into(), ImportedStr::new(ShortCStr::new(), trace));
        }
    }

    /// Set the `$(…)` stdout capture cap in bytes.
    pub fn set_capture_limit(&mut self, bytes: usize) {
        self.capture_limit = bytes;
    }

    /// Enter an `eval`/`source` frame, where inner commands must not update `_`.
    pub fn begin_eval(&mut self) {
        self.eval_depth += 1;
    }

    /// Leave an `eval`/`source` frame.
    pub fn end_eval(&mut self) {
        self.eval_depth = self.eval_depth.saturating_sub(1);
    }
}
