//! The variable-store setters of `state/setters.rs` (task #576 split): the
//! writers of `strings`/`exports` with their `ifs`/`optind` field syncs.

// mutants: -C --lib -C --test -C times -- -E 'test(/^state::/) | test(/^exec::search::/) | test(/^substitute::pattern::/) | test(/^tests::/) | test(/^child::bind::/) | test(/^intercept::tests::/) | binary(times)'

use super::ShellState;
use sys::ImportedStr;
use sys::Origin;
use sys::ShortCStr;
use sys::Trace;

impl ShellState {
    /// Store a shell string variable, keeping `ifs` in sync when the name is `IFS`
    /// and the `optind` counter in sync when it is `OPTIND` (task #589). An
    /// `OPTIND` assignment also resets the `optchar` cursor (task #590): bash
    /// measures every `OPTIND` assignment — parseable or not — as a re-read from
    /// the word's start. The sync serves a **user** assignment only: `getopts`
    /// writes its own counter and store row directly, and must not be routed
    /// through here (task #591 — `intercept/getopts/vars.rs`, the cursor trap).
    pub fn set_var(&mut self, name: ShortCStr, value: ImportedStr) {
        if name.eq_bytes(b"IFS") {
            self.ifs = value.value.clone();
        }
        if name.eq_bytes(b"OPTIND") {
            self.optind = crate::intercept::getopts::counter(&value.value, self.optind);
            self.optchar = 0;
        }
        self.strings.insert(name, value);
    }

    /// Remove a **shell** variable from every store that holds it: the string
    /// table, the array store (task #422), the export attribute, and the
    /// inherited `environ` row. bash 5.3.9
    /// (measured, task #290) takes the name away from `$name`, from `${name:-x}`
    /// and from the child environment at once, so the `environ` half is what
    /// makes `unset HOME` clear the fallback `substitute::resolve::var_value`
    /// reads and `exec::environ::get_environ` exports. `IFS` also goes back to
    /// the default split set, mirroring `set_var`'s sync, and `OPTIND` goes back to
    /// its default counter (task #589) — bash 5.3.9 measures an **unbound** name
    /// there (`unset OPTIND; echo "[$OPTIND]"` answers `[]`) while its scan still
    /// starts at argument 1, so only the counter keeps its seed.
    ///
    /// An unbound name is a silent no-op: bash validates no operand of `unset`
    /// at all (`unset 1abc`, `unset ""` are rc 0 with no effect). A plain
    /// `unset` operand reaches this through `super::functions::unset_name`,
    /// which tries the variable stores first and only then the function store
    /// (bash's rule, task #386).
    pub fn unset_var(&mut self, name: &ShortCStr) {
        if name.eq_bytes(b"IFS") {
            self.ifs = Self::default_ifs();
        }
        if name.eq_bytes(b"OPTIND") {
            self.optind = crate::intercept::getopts::DEFAULT_OPTIND;
            self.optchar = 0;
        }
        let _ = self.strings.remove(name);
        self.remove_str_array(name);
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
}
