use alloc::vec::Vec;

/// A segment of a script line extracted by the scanner.
pub(crate) enum Segment<'a> {
    /// Simple statement: the command bytes (`cmd`), its offset in the line
    /// (`off`), its heredoc body bytes (`bodies`, empty when the statement
    /// has no heredoc), the plain-`&` flag marking its **last** run to
    /// run asynchronously (`background`, task #326), and the flag saying one
    /// of its runs opens a mid-run construct whose closer never arrived
    /// (`midrun_open`, task #294), and whether its text ends on the operator's
    /// terminator so that the segments below it belong to the same statement
    /// (`join_next`, task #492).
    Statement {
        cmd: &'a [u8],
        off: usize,
        bodies: Vec<Vec<u8>>,
        background: bool,
        /// A mid-run construct of this run (`&& { …`, `&& while … do`,
        /// `| while … do`, `&& f() {`) has no closer: the scan skipped its
        /// span to the end of input, so this is the REPL's continuation
        /// signal (task #294) and inert for the script runner, whose parse
        /// layer owns the missing-closer diagnostic.
        midrun_open: bool,
        /// The run's own command line wrote a `<<` and ended on the operator's
        /// terminator (POSIX #2.9), so its body is a region on the lines below
        /// and the runner concatenates it with the segments it continues onto
        /// (`script/merge.rs`, task #492).
        join_next: bool,
        /// The newlines **inside `cmd`** at which the scan stepped over a physical
        /// line's end because the run stayed open on the operator's terminator
        /// (POSIX #2.9) — offsets into `cmd`, in scan order. The run the decision
        /// was made for is the one that spans them, and the runner writes each as a
        /// space so the chain it folds is one line for the parse layer
        /// (`script/merge/chain.rs`, task #492). A quoted word's newline and a
        /// comment's terminating newline are **not** here: they are data the parse
        /// layer keeps, and they stay newlines.
        joins: Vec<usize>,
    },
    /// A plain `&` whose split is invalid (nothing to terminate, `;`-separated
    /// target, or a `;`/`|`/`&` peek behind it — bash's "syntax error, nothing
    /// runs" family). `run_script` bails on it **before executing any**
    /// segment; it is never executed itself.
    AsyncInvalid,
    /// A stray `;;` outside a `case` clause (task #423): a `;;` pair in the
    /// statement list is bash's syntax error and it stops before running
    /// anything, so `run_script` bails on it at preflight — after the
    /// [`Segment::AsyncInvalid`] check (the order is pinned by `tests/async.rs`,
    /// whose rows keep their own message); it is never executed itself.
    UnexpectedSemiSemi,
    /// A closing keyword word in command position that closes **no** open block
    /// of its kind (task #436, the decision of [`crate::keywords::Pairing`]):
    /// bash's grammar refuses it, and because bash parses the whole input before
    /// running anything, `run_script` bails on it at preflight — after the
    /// [`Segment::AsyncInvalid`] and [`Segment::UnexpectedSemiSemi`] arms (the
    /// order is pinned by `tests/block_closers.rs`); it is never executed itself.
    /// `off` is the word's offset in the line, so the report can draw the caret.
    /// The **top-level** scan records it once per scanned text; the nested
    /// scorers read the same pairing for their score and report nothing.
    StrayCloser { off: usize },
    /// Block (`if … fi`, `while … done`, `case … esac`, `name() { … }`,
    /// `{ …; }`) spanning from `block_start` to `end_pos`.
    Block {
        block_start: usize,
        end_pos: usize,
        /// Whether the construct's **own** closer was found — the runner's and
        /// the `&`-split's signal, unchanged by the flag below.
        closed: bool,
        /// A `{ …; }` group whose own brace closed swallowed a statement tail
        /// holding an **unclosed** group (`{ echo a; } && { echo b`, task #401):
        /// this is the REPL's continuation signal and inert for the runner, whose
        /// parse layer owns the missing-closer diagnostic — exactly like
        /// [`Segment::Statement::midrun_open`] (task #294). A run-start span has
        /// no flush for the book's flag to land on, so the tail rides here; every
        /// other producer passes `false` and reports the tail through its own
        /// `closed`.
        midrun_open: bool,
        /// The block's own heredoc body bytes, appended to its text; empty
        /// when the block carries no `<<` operator, or when its bodies are
        /// interleaved with its lines (extension mode, see
        /// [`crate::segment::block::keyword::keyword_block`]).
        bodies: Vec<Vec<u8>>,
    },
}
