//! How the `$@` / `$*` arm lays the positional list into the word being built.
//!
//! Two shapes, two callers (task #319). The argument/word surfaces consume the
//! text with `substitute/split.rs::split_word`, so the arm joins where a
//! splitter consumes (`Join::Args`). A `case` **pattern** word is never split
//! and never globbed, and bash keeps only the **first expanded word** of it
//! (`execute_case_command`, bash-5.3 `execute_cmd.c`: the pattern is
//! `es->word->word`), so the arm must express the positional *word list* for
//! the caller to cut, and join only where bash itself joins (`Join::Pattern`).
//! LESSONS: "`$@` has two join rules".

/// The join mode of the `$@`/`$*` arm, threaded from the entry point that owns
/// the surface down to the arm; the arm is the only reader of `IFS` here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Join {
    /// The argument/word path (task #222): a quoted `"$@"` pushes its
    /// positionals adjacent with a word boundary between them; an unquoted
    /// `$@` and any `$*` join with the first `IFS` byte, a space when `IFS`
    /// is empty.
    Args,
    /// A `case` **pattern** word (task #319, all rows measured in bash 5.3.9):
    /// a quoted `"$@"` with a non-empty `IFS` is the adjacent push with
    /// boundaries, so the caller's boundary cut keeps only the first
    /// positional; with an empty `IFS` it **space-joins** and emits no
    /// boundary, so nothing is dropped; an unquoted `$@` **space-joins**
    /// whatever the `IFS` is; `$*` (quoted or not) joins with the first `IFS`
    /// byte and **concatenates** when the `IFS` is empty.
    Pattern,
}
