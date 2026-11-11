// mutants: -C --lib -C --test -C strict -- -E 'test(/^state::/) | binary(strict)'

use alloc::collections::{BTreeMap, VecDeque};
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
    /// Shell string arrays: `name=(w1 w2 …)`'s elements, sparse and sorted by
    /// index (bash's `a=(); a[5]=x` answers `${a[@]}` = `x`, `${#a[@]}` = `1`).
    /// Unprefixed names, so the `strings` namespace; the `%` fd arrays are the
    /// store above (task #422).
    pub(crate) str_arrays: HashMap<ShortCStr, BTreeMap<usize, ImportedStr>>,
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
    /// True when word 0's expansion produced **no field**, so the command name is
    /// omitted (POSIX #2.9.1) and the run side must not look anything up. Set by
    /// `command_word::with_fields`' zero-field arm — a *quoted* word that expands
    /// to nothing yields one empty field and stays a lookup (bash 127) — and reset
    /// at the top of `command_word::expand`, i.e. per command, per pipeline stage.
    pub(crate) word0_vanished: bool,
    /// The status of the last command substitution performed **for the command
    /// being built** (`None` = none ran): the omitted-name command's status is
    /// this, or 0 (POSIX #2.9.1). Recorded at the `cmd_subst` reap site — the one
    /// path both `$( … )` and `` ` … ` `` go through — and reset with
    /// [`ShellState::word0_vanished`].
    pub(crate) last_substitution: Option<WaitStatus>,
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
    /// True while the command being executed runs inside a part of a `&&`/`||`
    /// list that bash exempts from errexit (every part except the command
    /// following the final operator), or inside an `if`/`elif`/`while`/`until`
    /// condition (task #491) — the execution context set by
    /// [`crate::cond::errexit::gated`]; `should_exit` consults it.
    pub(crate) errexit_gated: bool,
    pub(crate) eval_depth: u32,
    pub(crate) ifs: ShortCStr,
    pub(crate) options: u32,
    /// The REPL's interactive flag: true when its stdin is a terminal (bash's
    /// "interactive shell"). Seeds the default `ignoreeof` and gates the EOF
    /// policy; false in `-c` and script-file modes.
    pub(crate) interactive: bool,
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

    /// The fresh shell. Every field's seed is spelled out once, in
    /// [`ctor::build`].
    pub fn new() -> Self {
        ctor::build()
    }
}

impl Default for ShellState {
    fn default() -> Self {
        Self::new()
    }
}

mod arrays;
mod ctor;
mod cwd;
pub(crate) mod fds;
pub(crate) mod frames;
mod functions;
pub(crate) mod jobs;
mod options;
mod setters;
mod shell_arrays;

#[cfg(test)]
mod tests;
