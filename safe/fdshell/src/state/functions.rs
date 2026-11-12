// mutants: -C --lib -C --test -C unset -- -E 'test(/^state::/) | test(/^tests::run::/) | binary(unset)'

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

    /// True for exactly the stores [`ShellState::unset_var`] owns — the string
    /// table, the array store (task #422: `a=(1 2); unset a` takes the array away,
    /// so a name that lives **only** there is still a variable), the export
    /// attribute and the inherited `environ` row — and **nothing else**: the
    /// single-name `IFS` field is a mirror of the entry, not a store, so it answers
    /// nothing here. Which makes `IFS` a variable *while a store holds it*: the
    /// startup seed (task #388) is what holds it, so a function named `IFS` survives
    /// the first `unset IFS` (bash measures `F`, task #386), while the **second** —
    /// no store holds the name any more — takes the function with it (bash:
    /// `command not found`, task #512).
    fn is_variable(&self, name: &ShortCStr) -> bool {
        self.strings.contains_key(name)
            || self.str_arrays.contains_key(name)
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
