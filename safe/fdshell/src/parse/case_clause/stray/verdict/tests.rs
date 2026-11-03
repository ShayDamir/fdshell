#![allow(clippy::unwrap_used)]

use super::{arith_seed, classify, opens_groups_at};
use crate::keywords::Place;
use crate::mask::MaskBit;
use crate::parse::token::tokenize_statement;
use alloc::vec;

/// [`classify`] takes the place the walk read the predecessor word on, so each
/// arm is pinned with the place and a region whose judged token is a span-clean
/// `)` — the classification's own contract, row by row (the walk's gates are
/// pinned in [`super::tests`]).
#[test]
fn each_arm_of_the_classification_answers_its_own_rows() {
    let region = tokenize_statement(b"x)").unwrap();
    let paren = 1;
    // Word-list places: the arm's own pattern paren passes (the nested clause
    // behind `in`, the `for` list).
    assert_eq!(classify(Place::Pattern, &region, paren), None);
    assert_eq!(classify(Place::Words, &region, paren), None);
    // Task #300: a command-position predecessor word that is not an operator
    // token is the clause pattern paren of the missing `;;`.
    assert_eq!(
        classify(Place::Command, &region, paren),
        Some(super::Stray::ClausePattern),
        "bash rc 2"
    );
    // The argument place (an ordinary word leaves [`Place::Data`]) is task
    // #419's family: the command list cannot take the paren (bash rc 2).
    assert_eq!(
        classify(Place::Data, &region, paren),
        Some(super::Stray::CloseParen),
        "bash rc 2"
    );
    // An operator predecessor: the `;` before the paren is the operator token,
    // so the clause-pattern arm's operator test fails and the new arm answers
    // (`;` + `)` as one region, the window family).
    let operator = tokenize_statement(b";)").unwrap();
    assert_eq!(
        classify(Place::Command, &operator, 1),
        Some(super::Stray::CloseParen),
        "bash rc 2"
    );
    // No predecessor at all: the `i > 0` guard belongs to task #300's arm only,
    // so an arm region that *starts* at the paren is judged by this one.
    let starts = tokenize_statement(b")").unwrap();
    assert_eq!(
        classify(Place::Command, &starts, 0),
        Some(super::Stray::CloseParen),
        "bash rc 2"
    );
}

/// The place gate on the opener count (task #419): a `(` opens a group only
/// where bash's grammar recognises it as an operator — command position. The
/// counter's own byte contract is pinned in [`super::tests`]; this is the gate.
#[test]
fn the_place_gate_asks_the_counter_only_in_command_position() {
    let mask = |n: usize| vec![MaskBit::Unquoted; n];
    let word = sys::ShortCStr::from(c"m=(1");
    let bytes = word.as_bytes().unwrap();
    // The same word: an opener in command position, nothing in argument
    // position (`echo m=(1) ;;`, where the `)` lands at depth 0).
    assert_eq!(opens_groups_at(Place::Command, bytes, &mask(4)), 1);
    assert_eq!(opens_groups_at(Place::Data, bytes, &mask(4)), 0);
    assert_eq!(opens_groups_at(Place::Words, bytes, &mask(4)), 0);
    // A quoted paren is word text in either place.
    let quoted = vec![
        MaskBit::Unquoted,
        MaskBit::Unquoted,
        MaskBit::Quoted,
        MaskBit::Unquoted,
    ];
    assert_eq!(opens_groups_at(Place::Command, bytes, &quoted), 0);
    // `((` is the arithmetic region's own rule, not the counter's.
    let arith = sys::ShortCStr::from(c"((1");
    assert_eq!(
        opens_groups_at(Place::Command, arith.as_bytes().unwrap(), &mask(3)),
        0
    );
}

/// The seed: a `((` word **outside** the region — the clause's first pattern,
/// stripped byte by byte by the pattern walk (task #334) — owns two closers, and
/// the production spent the first, so the region walk starts owning the other
/// (task #419: `case abc in ((a*)) echo m;; *) echo n;; esac` stays accepted,
/// while `case abc in (a*) ) echo m;; *) echo n;; esac` is refused).
#[test]
fn the_double_paren_production_word_arms_the_walks_second_closer() {
    // tokens: case, abc, in, `((a*`, `)`, `)`, echo, m, `;;`, `*`, `)`, echo, n, `;;`, esac
    let tokens = tokenize_statement(b"case abc in ((a*)) echo m;; *) echo n;; esac").unwrap();
    assert_eq!(arith_seed(&tokens, 5), 1);
    // A single leading `(` is the production's, not the region's: no seed, and
    // the region's leading `)` is judged (bash rc 2 on both rows).
    let tokens = tokenize_statement(b"case abc in (a*) ) echo m;; *) echo n;; esac").unwrap();
    assert_eq!(arith_seed(&tokens, 5), 0);
    // `take_open_paren`'s discriminator is the mask: a quoted leading `(` is
    // word text, the strip never ran, so the region owns nothing.
    let tokens = tokenize_statement(b"case abc in \"((\"a*) echo m;; esac").unwrap();
    assert_eq!(arith_seed(&tokens, 5), 0);
    // Nothing in front of the region: no word, no seed.
    let tokens = tokenize_statement(b"case abc in ((a*)) esac").unwrap();
    assert_eq!(arith_seed(&tokens, 1), 0);
}
