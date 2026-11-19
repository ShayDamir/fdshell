// mutants: -C --lib -- -E 'test(/^numarg::/)'

//! One scanner for bash's numeric-argument grammars; the grammars are data.
//!
//! bash routes each builtin's numeric word through one of a few parsers, and
//! they differ in opposite directions, so Rust's `FromStr` — a fourth grammar
//! that belongs to no builtin — serves none of them. Each call site names the
//! grammar bash uses for it; range rules stay at the call site.

pub(crate) use scan::{Grammar, LEGAL_NUMBER, ULIMIT_VALUE, decimal};
pub(crate) use seconds::seconds_ms;

mod scan;
mod seconds;

#[cfg(test)]
mod tests;
