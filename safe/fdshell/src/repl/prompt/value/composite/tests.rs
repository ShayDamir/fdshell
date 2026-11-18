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
    // The third part answers nothing, as the seam's real third part does for
    // this escape.
    let first = fake(ValueEscape::Username, b"first");
    let second = fake(ValueEscape::Username, b"second");
    let third = fake(ValueEscape::Jobs, b"ignored");
    let providers = Providers::new(&first, &second, &third);
    assert_eq!(
        providers.value(ValueEscape::Username),
        Some(b"first".to_vec())
    );
}

#[test]
fn a_silent_provider_lets_the_next_answer() {
    // The seam's real shape: one fake per owner family (shell, clock, cwd),
    // each answering its own escape and `None` for the others' — every part
    // is reached through the composite.
    let shell = fake(ValueEscape::Username, b"the-user");
    let clock = fake(ValueEscape::Time, b"13:41:42");
    let cwd = fake(ValueEscape::WorkingDir, b"/the/path");
    let providers = Providers::new(&shell, &clock, &cwd);
    assert_eq!(
        providers.value(ValueEscape::Username),
        Some(b"the-user".to_vec())
    );
    assert_eq!(
        providers.value(ValueEscape::Time),
        Some(b"13:41:42".to_vec())
    );
    assert_eq!(
        providers.value(ValueEscape::WorkingDir),
        Some(b"/the/path".to_vec())
    );
    // Every part is *reachable* through the composite, in its own slot: a
    // composite that skipped the last one would answer `None` for the cwd
    // fake's escape, which is what keeps the three-part wiring killable.
}

#[test]
fn nothing_answers() {
    // An escape no fake owns is `None` through the composite — decision D2's
    // fallback, drawn literally by the renderer (what `\w` answers on an
    // origin that names no path, pinned in `cwd/tests.rs`).
    let shell = fake(ValueEscape::Username, b"x");
    let clock = fake(ValueEscape::Time, b"y");
    let cwd = fake(ValueEscape::WorkingDir, b"z");
    let providers = Providers::new(&shell, &clock, &cwd);
    assert_eq!(providers.value(ValueEscape::Date), None);
}
