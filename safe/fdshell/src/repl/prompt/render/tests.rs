#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;

use crate::repl::prompt::decode::decode;
use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::{NoValues, Values};

/// A provider answering for exactly one escape: the seam's substitution arm,
/// with every other placeholder keeping its literal text.
struct Shout;

impl Values for Shout {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        (esc == ValueEscape::Username).then(|| b"me".to_vec())
    }
}

#[test]
fn the_renderer_concatenates_every_run_as_bash_draws_it() {
    // The bash-measured byte stream for `a\[…\]C` (plan §3): the region is
    // drawn and the toggling markers consumed.
    assert_eq!(
        super::render(&decode(br"a\[\033[31mB\033[0m\]C"), &NoValues),
        b"a\x1b[31mB\x1b[0mC",
    );
}

#[test]
fn a_provider_substitutes_its_value_and_the_rest_stays_literal() {
    assert_eq!(super::render(&decode(br"\u\v"), &Shout), b"me\\v");
}

#[test]
fn no_values_renders_the_default_prompt_byte_identically() {
    // Decision D2: the default `PS1` is all value escapes and the no-values
    // provider answers `None` — which is what keeps the verbatim pins green.
    assert_eq!(
        super::render(&decode(br"\s-\v\$ "), &NoValues),
        br"\s-\v\$ ",
    );
}
