use sys::ShortCStr;

use super::ShellState;

impl ShellState {
    /// The delete path of a plain `unset` operand: **variable first, function
    /// second** — bash 5.3.9 (measured, task #386), and `bash -c 'help unset'`
    /// says it outright: "Without options, unset first tries to unset a
    /// variable, and if that fails, tries to unset a function". A name both a
    /// variable and a function keeps its function, because the variable is what
    /// the priority reaches first; a name in neither store is a silent no-op.
    /// The order is only observable from a name in **one** store alone, so the
    /// pins name one store each (`state/tests.rs`, `tests/unset.rs`).
    pub(crate) fn unset_name(&mut self, name: &ShortCStr) {
        if self.is_variable(name) {
            self.unset_var(name);
        } else {
            self.unset_function(name);
        }
    }

    /// True for every store [`ShellState::unset_var`] owns — the string table,
    /// the export attribute, the inherited `environ` row — plus the single-name
    /// `IFS` sync, which bash also counts as a variable: a function named `IFS`
    /// **survives** `unset IFS` (measured, task #386). The startup seed already
    /// puts `IFS` in the string table, so this clause is what answers when the
    /// seed is gone — the second `unset IFS`, measured as task #512.
    fn is_variable(&self, name: &ShortCStr) -> bool {
        name.eq_bytes(b"IFS")
            || self.strings.contains_key(name)
            || self.exports.contains_key(name)
            || self.environ.iter().any(|(k, _)| k == name)
    }

    /// Take the name away from the function store; a name that is not a function
    /// is a silent no-op. The removal is **global** — definitions are not
    /// frame-scoped, so the `local` frame that performed it does not undo it at
    /// return (`g(){ unset f; }; g; f` leaves `f` gone, and bash measures that).
    fn unset_function(&mut self, name: &ShortCStr) {
        let _ = self.functions.remove(name);
    }
}
