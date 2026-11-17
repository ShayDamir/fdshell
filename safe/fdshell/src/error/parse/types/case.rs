//! The `case` family of `ParseError`, kept out of `types.rs` so the enums stay
//! under the line budget; the facade (`error/parse.rs`) re-exports this enum and
//! `ParseError` forwards it with the `Case` variant.

/// [CaseError] Parser errors, the `case` family (owned by `parse::case_block`
/// and `parse::case_clause`).
// Debug needed for impl Error (trait bound), not for display.
#[derive(displaydoc::Display, Debug)]
pub(crate) enum CaseError {
    /// case: missing 'in'
    CaseMissingIn,
    /// case: empty pattern
    CaseEmptyPattern,
    /// case: missing 'esac'
    CaseMissingEsac,
    /// case: missing ')'
    CaseMissingCloseParen,
    /// case: missing ';;' between clauses
    CaseMissingSemiSemi,
    /// case: empty word
    CaseEmptyWord,
    /// case: ';' before an empty clause list
    CaseEmptyClauseList,
    /// case: '(' and ')' are control operators here (quote them)
    CaseWordOperator,
    /// case arm: unexpected ')' — a command list cannot take a paren
    CaseArmUnexpectedCloseParen,
}

impl core::error::Error for CaseError {}
