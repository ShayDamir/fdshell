// mutants: -C --lib -C --test -C times -- -E 'test(/^state::/) | test(/^exec::search::/) | test(/^substitute::pattern::/) | test(/^tests::/) | test(/^child::bind::/) | test(/^intercept::tests::/) | binary(times)'

mod vars;

use alloc::collections::VecDeque;

use sys::ImportedStr;
use sys::LocalFd;
use sys::Origin;
use sys::Trace;
use sys::siginfo::WaitStatus;

use super::{FdVar, ShellState};

impl ShellState {
    /// Drop the first `n` of `$1..$N`. `$0` is never shifted (bash).
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

    /// Set `$0`, the program name. Only the CLI seeds write it: `set --`,
    /// function calls, `source` and `shift` never rebind it (bash).
    pub fn set_program_name(&mut self, name: ImportedStr) {
        self.program_name = name;
    }

    pub fn set_last_exit(&mut self, code: i32) {
        self.last_status = WaitStatus::Exited(code);
    }

    /// Fold a reaped child's CPU times (microseconds) into the shell's
    /// children totals for `times`.
    pub fn add_child_times(&mut self, times: sys::getrusage::CpuTimes) {
        self.child_utime = self.child_utime.saturating_add(times.utime);
        self.child_stime = self.child_stime.saturating_add(times.stime);
    }

    pub fn set_shell_sock(&mut self, sock: LocalFd) {
        self.shell_sock = Some(sock);
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
