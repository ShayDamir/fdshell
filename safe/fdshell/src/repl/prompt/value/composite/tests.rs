#![allow(clippy::unwrap_used)]

use alloc::vec::Vec;

use crate::repl::prompt::segment::ValueEscape;
use crate::repl::prompt::value::Values;
use crate::repl::prompt::value::composite::Providers;

/// A fake answering exactly one escape with fixed bytes — the shape the seam
/// guarantees of every provider, `None` for everything else.
struct Fake {
    mine: ValueEscape,
    bytes: Vec<u8>,
}

impl Values for Fake {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        (esc == self.mine).then(|| self.bytes.clone())
    }
}

fn fake(mine: ValueEscape, bytes: &[u8]) -> Fake {
    Fake {
        mine,
        bytes: bytes.to_vec(),
    }
}

#[test]
fn the_first_provider_that_answers_wins() {
    // Both fakes answer the *same* escape with different bytes, so the order
    // is observable — a swap of the parts is otherwise an equivalent mutant.
    let first = fake(ValueEscape::Username, b"first");
    let second = fake(ValueEscape::Username, b"second");
    let providers = Providers::new(&first, &second);
    assert_eq!(
        providers.value(ValueEscape::Username),
        Some(b"first".to_vec())
    );
}

#[test]
fn a_silent_provider_lets_the_next_answer() {
    // The seam's real shape: a shell-shaped fake and a clock-shaped fake, each
    // answering its own escape and `None` for the other's.
    let shell = fake(ValueEscape::Username, b"the-user");
    let clock = fake(ValueEscape::Time, b"13:41:42");
    let providers = Providers::new(&shell, &clock);
    assert_eq!(
        providers.value(ValueEscape::Username),
        Some(b"the-user".to_vec())
    );
    assert_eq!(
        providers.value(ValueEscape::Time),
        Some(b"13:41:42".to_vec())
    );
}

#[test]
fn nothing_answers() {
    // An escape neither fake owns is `None` through the composite — decision
    // D2's fallback, drawn literally by the renderer.
    let shell = fake(ValueEscape::Username, b"x");
    let clock = fake(ValueEscape::Time, b"y");
    let providers = Providers::new(&shell, &clock);
    assert_eq!(providers.value(ValueEscape::Date), None);
}
