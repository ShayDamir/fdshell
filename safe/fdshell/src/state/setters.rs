use alloc::collections::VecDeque;

use sys::ImportedStr;
use sys::LocalFd;
use sys::Origin;
use sys::ShortCStr;
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

    /// Store a shell string variable, keeping `ifs` in sync when the name is `IFS`.
    pub fn set_var(&mut self, name: ShortCStr, value: ImportedStr) {
        if name.eq_bytes(b"IFS") {
            self.ifs = value.value.clone();
        }
        self.strings.insert(name, value);
    }

    /// Remove a **shell** variable from every store that holds it: the string
    /// table, the export attribute, and the inherited `environ` row. bash 5.3.9
    /// (measured, task #290) takes the name away from `$name`, from `${name:-x}`
    /// and from the child environment at once, so the `environ` half is what
    /// makes `unset HOME` clear the fallback `substitute::resolve::var_value`
    /// reads and `exec::environ::get_environ` exports. `IFS` also goes back to
    /// the default split set, mirroring `set_var`'s sync.
    ///
    /// An unbound name is a silent no-op: bash validates no operand of `unset`
    /// at all (`unset 1abc`, `unset ""` are rc 0 with no effect).
    pub fn unset_var(&mut self, name: &ShortCStr) {
        if name.eq_bytes(b"IFS") {
            self.ifs = Self::default_ifs();
        }
        let _ = self.strings.remove(name);
        let _ = self.exports.remove(name);
        self.environ.retain(|(k, _)| k != name);
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
