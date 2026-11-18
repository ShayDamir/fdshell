// mutants: -C --lib -C --test -C repl -- -E 'test(/^repl::/) | binary(repl)'

//! The composition that makes both value providers reachable from `resolve`:
//! the renderer takes one `&dyn Values`, so the composite asks each part in
//! turn and the first `Some` wins — the order is pinned by
//! `the_first_provider_that_answers_wins` because a swap is otherwise an
//! equivalent mutant.
//!
//! The parts are references because the tests name fakes; production builds
//! the pair at the call site in `prompt.rs`, where the owners live, from
//! `shell.rs`'s and `clock.rs`'s providers.

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;

/// The renderer's single provider, assembled from the providers that own the
/// escapes, in answer order.
pub(crate) struct Providers<'a> {
    parts: [&'a dyn Values; 2],
}

impl<'a> Providers<'a> {
    pub(crate) fn new(first: &'a dyn Values, second: &'a dyn Values) -> Self {
        Self {
            parts: [first, second],
        }
    }
}

impl Values for Providers<'_> {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        self.parts.into_iter().find_map(|part| part.value(esc))
    }
}

#[cfg(test)]
mod tests;
