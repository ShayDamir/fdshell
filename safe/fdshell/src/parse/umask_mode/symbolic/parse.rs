// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::umask_mode::/) | binary(umask)'

//! `SymbolicMode::parse` (task #554): bash's `parse_symbolic_mode` walk, one clause
//! at a time — the `who` letters, one operator, one action run, then the terminator:
//! the end of the word, a `,` (the next clause, `who` parsed again), or another
//! operator (the same clause, the same `who`). The two byte classes live in
//! `letters`; the AST is what the parent module evaluates, and nothing here reads the
//! mask, so the parser stays side-effect free.

mod letters;

use crate::error::parse::ParseError;
use crate::parse::umask_mode::symbolic::{Action, Clause, Op, Step, SymbolicMode};
use alloc::vec::Vec;
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

/// The whole mode, as the parent's `apply` wants it.
impl SymbolicMode {
    pub(crate) fn parse(word: &ShortCStr) -> Result<Self, Report<ParseError>> {
        let bytes = word.as_bytes().change_context(ParseError::Never)?;
        let mut clauses: Vec<Clause> = Vec::new();
        let mut actions: Vec<Action> = Vec::new();
        let mut who = 0u32;
        let mut at = 0usize;
        loop {
            while let Some(bit) = bytes.get(at).copied().and_then(letters::who_bit) {
                who |= bit;
                at += 1;
            }
            let clause_who = if who == 0 { 0o777 } else { who };
            let end = loop {
                let op = operator(bytes, word, at)?;
                at += 1;
                let mut perm: Vec<Step> = Vec::new();
                while let Some(step) = bytes.get(at).copied().and_then(letters::perm_step) {
                    perm.push(step);
                    at += 1;
                }
                actions.push(Action { op, perm });
                match bytes.get(at).copied() {
                    None => break End::Word,
                    Some(b',') => {
                        at += 1;
                        break End::Clause;
                    }
                    Some(b'+' | b'-' | b'=') => continue,
                    Some(_) => {
                        bail!(ParseError::InvalidUmaskSymbolicCharacter {
                            ch: name_at(word, at)
                        })
                    }
                }
            };
            clauses.push(Clause {
                who: clause_who,
                actions,
            });
            actions = Vec::new();
            who = 0;
            if matches!(end, End::Word) {
                break;
            }
        }
        Ok(Self { clauses })
    }
}

/// How a clause ended: the end of the word, or a `,` starting another clause.
enum End {
    Word,
    Clause,
}

/// The operator bash's `start_op:` label insists on. Any other byte — including the
/// end of the word, which bash reports as the empty character — is the error.
fn operator(bytes: &[u8], word: &ShortCStr, at: usize) -> Result<Op, Report<ParseError>> {
    match bytes.get(at).copied() {
        Some(b'+') => Ok(Op::Add),
        Some(b'-') => Ok(Op::Remove),
        Some(b'=') => Ok(Op::Assign),
        _ => bail!(ParseError::InvalidUmaskSymbolicOperator {
            op: name_at(word, at)
        }),
    }
}

/// The offending byte, as the message prints it; the end of the word prints as the
/// empty name, which is how bash renders its `\0` there.
fn name_at(word: &ShortCStr, at: usize) -> ShortCStr {
    word.get(at..at + 1).unwrap_or_default()
}
