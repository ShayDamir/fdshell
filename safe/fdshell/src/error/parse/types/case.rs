//! The `case` family of `ParseError`, kept out of `types.rs` so the enums stay
//! under the line budget; the facade (`error/parse.rs`) re-exports this enum and
//! `ParseError` forwards it with the `Case` variant.

/// [CaseError] Parser errors, the `case` family (owned by `parse::case_block`
/// and `parse::case_clause`).
// Debug needed for impl Error (trait bound), not for display.
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CaseError {
    /// case: missing 'in'
    MissingIn,
    /// case: empty pattern
    EmptyPattern,
    /// case: missing 'esac'
    MissingEsac,
    /// case: missing ')'
    MissingCloseParen,
    /// case: missing ';;' between clauses
    MissingSemiSemi,
    /// case: empty word
    EmptyWord,
    /// case: ';' before an empty clause list
    EmptyClauseList,
    /// case: '(' and ')' are control operators here (quote them)
    WordOperator,
    /// case arm: unexpected ')' — a command list cannot take a paren
    ArmUnexpectedCloseParen,
}

impl core::error::Error for CaseError {}
