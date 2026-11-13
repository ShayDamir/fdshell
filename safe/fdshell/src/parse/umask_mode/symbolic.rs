// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::umask_mode::/) | binary(umask)'

//! The symbolic mode's AST, its evaluation, and the `-S` line (task #554) — the
//! rules of bash's `builtins/umask.def`, pinned by bash 5.3.9 measurements.
//!
//! bash computes everything on the complement of the mask ("All work is done with
//! the complement of the umask"): the allowed bits start at `!mask & 0777`, each
//! clause moves them, and the result is complemented back. Two of its rules are
//! easy to get wrong and are pinned in `tests.rs`: a copy spec (`u`/`g`/`o` inside
//! an action run) **replaces** the permissions the run had accumulated, and both it
//! and `X` read the **initial** allowed bits — never the bits an earlier clause
//! moved. `s` and `t` add only bits above `0777`, which `perm &= who` erases: they
//! are accepted and do nothing.

mod parse;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The parsed mode: comma-separated clauses, each with its own `who`.
#[derive(Debug, PartialEq)]
pub(crate) struct SymbolicMode {
    clauses: Vec<Clause>,
}

/// One clause: the classes it names (all three when it names none) and its
/// operators — an operator written after an action run restarts the same clause,
/// keeping its `who`.
#[derive(Debug, PartialEq)]
struct Clause {
    who: u32,
    actions: Vec<Action>,
}

/// One operator and the permissions its action run accumulated.
#[derive(Debug, PartialEq)]
struct Action {
    op: Op,
    perm: Vec<Step>,
}

#[derive(Debug, PartialEq)]
enum Op {
    Add,
    Remove,
    Assign,
}

/// One character of an action run: static bits, a copy spec (the class whose bits
/// it copies), or `X`.
#[derive(Debug, PartialEq, Clone)]
enum Step {
    Bits(u32),
    Copy(u32),
    X,
}

impl SymbolicMode {
    /// Apply the mode to `mask` and answer the new mask (bash's `symbolic_umask`).
    pub(crate) fn apply(&self, mask: u32) -> u32 {
        let initial = !mask & 0o777;
        let mut bits = initial;
        for clause in &self.clauses {
            for action in &clause.actions {
                let perm = expand(action, initial, clause.who);
                bits = match action.op {
                    Op::Add => bits | perm,
                    Op::Remove => bits & !perm,
                    // `expand` masks `perm` by `clause.who`, so this sum's operands are
                    // disjoint and the sum is exact — the bitwise spelling would have no
                    // test that can tell it from its `^` twin, so the sum is the honest
                    // form. Pinned by the `a=rwr` rows.
                    Op::Assign => (bits & !clause.who) + perm,
                };
            }
        }
        !bits & 0o777
    }
}

/// The permissions of one operator, in the order the action run wrote them —
/// static bits add, `X` adds when the initial bits have an exec bit, a copy
/// replaces — and the whole run masked by the clause's `who`.
fn expand(action: &Action, initial: u32, who: u32) -> u32 {
    let mut perm = 0;
    for step in &action.perm {
        perm = match *step {
            Step::Bits(bits) => perm | bits,
            Step::Copy(class) => copy(initial, class),
            Step::X => perm | x_perm(initial),
        };
    }
    perm & who
}

/// bash's `copyuser`/`copygroup`/`copyother`: the class's three bits, each expanded
/// to all three classes; `class` is that class's mask (`0700`, `0070`, `0007`).
fn copy(initial: u32, class: u32) -> u32 {
    let mut perm = 0;
    for bit in [0o444, 0o222, 0o111] {
        if initial & (bit & class) != 0 {
            perm |= bit;
        }
    }
    perm
}

/// `X`: bash's "no-op if original mask doesn't include execute bits" (its `chmod`
/// directory test above cannot apply to a umask).
fn x_perm(initial: u32) -> u32 {
    if initial & 0o111 == 0 { 0 } else { 0o111 }
}

/// The body of a `-S` line, bash's `print_symbolic_umask`: a letter per allowed bit.
pub(crate) fn symbolic_text(mask: u32) -> String {
    let (u, g, o) = (class(mask, 0o700), class(mask, 0o070), class(mask, 0o007));
    format!("u={u},g={g},o={o}")
}

/// One class of that line: `r`, `w`, `x` for every bit the mask leaves clear.
fn class(mask: u32, class: u32) -> String {
    let mut text = String::new();
    for (bit, letter) in [(0o444, 'r'), (0o222, 'w'), (0o111, 'x')] {
        if mask & (bit & class) == 0 {
            text.push(letter);
        }
    }
    text
}

#[cfg(test)]
mod tests;
