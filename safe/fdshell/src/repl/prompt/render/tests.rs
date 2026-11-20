#![allow(clippy::unwrap_used)]

use alloc::vec;
use alloc::vec::Vec;

use sys::ImportedStr;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::repl::prompt::decode::Markers;
use crate::repl::prompt::decode::decode;
use crate::repl::prompt::render::PromptVars;
use crate::repl::prompt::segment::{Segment, SegmentKind, ValueEscape};
use crate::repl::prompt::value::{NoValues, Values};
use crate::state::ShellState;

/// A provider answering for exactly one escape: the seam's substitution arm,
/// with every other placeholder keeping its literal text.
struct Shout;

impl Values for Shout {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        (esc == ValueEscape::Username).then(|| b"me".to_vec())
    }
}

/// A provider whose answer carries the bytes an expansion stage acts on: a `$`
/// naming a set variable and a `~` with a `HOME` set.
struct Dollarish;

impl Values for Dollarish {
    fn value(&self, esc: ValueEscape) -> Option<Vec<u8>> {
        (esc == ValueEscape::Username).then(|| b"me$hello~".to_vec())
    }
}

/// A state with the variable a run can name (`hello`), and `HOME` so a `~` would
/// move if the bytes of a provider answer were scanned (they are not).
fn cell() -> ForkCell<ShellState> {
    let cell = ForkCell::new(ShellState::new());
    for (name, value) in [(c"hello", c"V"), (c"HOME", c"/h/o/m/e")] {
        cell.borrow_mut()
            .unwrap()
            .strings
            .insert(ShortCStr::from(name), ImportedStr::shell(value.into()));
    }
    cell
}

/// The bytes `resolve` draws from a record — the concatenation of its runs.
fn drawn(
    record: &[Segment],
    values: &dyn Values,
    policy: PromptVars,
    cell: &ForkCell<ShellState>,
) -> Vec<u8> {
    super::render(record, values, policy, cell)
        .iter()
        .flat_map(|run| run.bytes.iter())
        .copied()
        .collect()
}

/// Decode and render one prompt, the way `resolve` does.
fn run(input: &[u8], values: &dyn Values, policy: PromptVars) -> Vec<u8> {
    drawn(&decode(input, Markers::Bytes), values, policy, &cell())
}

#[test]
fn the_renderer_concatenates_every_run_as_bash_draws_it() {
    // The bash-measured byte stream for `a\[…\]C` (task #616): the region is
    // drawn and the toggling markers consumed.
    assert_eq!(
        run(br"a\[\033[31mB\033[0m\]C", &NoValues, PromptVars::On),
        b"a\x1b[31mB\x1b[0mC",
    );
}

#[test]
fn a_provider_substitutes_its_value_and_the_rest_stays_literal() {
    assert_eq!(run(br"\u\v", &Shout, PromptVars::On), b"me\\v");
}

#[test]
fn no_values_renders_the_default_prompt_byte_identically() {
    // Decision D2: the default `PS1` is all value escapes and the no-values
    // provider answers `None` — which is what keeps the verbatim pins green.
    assert_eq!(run(br"\s-\v\$ ", &NoValues, PromptVars::On), br"\s-\v\$ ",);
}

#[test]
fn a_provider_answer_is_data_and_never_a_trigger() {
    // D-620-2: a provider's bytes are pasted verbatim, never scanned — the rule
    // that reproduces bash's measured `\$v` row. Scanned, this answer would draw
    // `meworld/h/o/m/e`.
    assert_eq!(run(br"\u", &Dollarish, PromptVars::On), b"me$hello~");
}

#[test]
fn promptvars_off_skips_the_expansion_only() {
    // Measured (`shopt -u promptvars` + `a"b$v"c ` → verbatim): the decoder has
    // already run — task #616's `\a` pins stay put — and quote removal is part of
    // the skipped expansion.
    assert_eq!(
        run(br#"a\"b$hello\"c"#, &NoValues, PromptVars::Off),
        br#"a\"b$hello\"c"#
    );
    assert_eq!(
        run(br#"a\"b$hello\"c"#, &NoValues, PromptVars::On),
        b"a\"bV\"c"
    );
}

#[test]
fn the_record_keeps_its_kinds_with_final_bytes() {
    // D-620-1: epic #198 reads this record to know which bytes are visible, so
    // with `promptvars` on its bytes are the expanded ones and its kinds are the
    // decoder's.
    assert_eq!(
        super::render(
            &decode(br"a\[a$hello\]C", Markers::Bytes),
            &NoValues,
            PromptVars::On,
            &cell()
        ),
        vec![
            Segment {
                kind: SegmentKind::Visible,
                bytes: b"a".to_vec(),
            },
            Segment {
                kind: SegmentKind::NonPrinting,
                bytes: b"aV".to_vec(),
            },
            Segment {
                kind: SegmentKind::Visible,
                bytes: b"C".to_vec(),
            },
        ],
    );
}
