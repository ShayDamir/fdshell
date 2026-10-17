//! Unit pins for the shared command-position rule: every [`Place`] transition
//! of [`after`], every gate arm of [`scores`], and the asymmetries the rule
//! encodes (opener vs closer, `case` list vs argument, brace carve-out,
//! `time [-p]` transparency).

use super::{Glue, Place, after, scores};
use crate::keywords::first_word_end;

/// `after` on the whole word: `word_end` sits just past it, outside a cond
/// pair. `glue` marks the word's start in a pair (see [`super::after`]).
fn after_word(word: &[u8], place: Place) -> Place {
    after_word_glue(word, place, Glue::None)
}

fn after_word_glue(word: &[u8], place: Place, glue: Glue) -> Place {
    after(word, word, word.len(), place, glue)
}

/// `scores` on the whole word.
fn scores_word(word: &[u8], place: Place) -> bool {
    scores(word, word, word.len(), place)
}

#[test]
fn command_starters_move_the_place_to_command() {
    for word in [
        b"".as_slice(),
        b";",
        b"&&",
        b"||",
        b"&",
        b"(",
        b")",
        b"if",
        b"while",
        b"until",
        b"then",
        b"else",
        b"elif",
        b"do",
        b"{ ",
    ] {
        assert_eq!(
            after_word(word, Place::Data),
            Place::Command,
            "the word behind {word:?} starts a command"
        );
        assert_eq!(after_word(word, Place::Pattern), Place::Command);
    }
}

/// The `case` clause `)` is usually glued to its pattern (`x)`), so the
/// transition reads the word's *last* byte.
#[test]
fn glued_case_clause_close_moves_the_place_to_command() {
    assert_eq!(after_word(b"x)", Place::Data), Place::Command);
    assert_eq!(after_word(b"a b c)", Place::Pattern), Place::Command);
    // A substitution span's `)` too: `x=$(a)` ends in command position.
    assert_eq!(after_word(b"x=$(a)", Place::Data), Place::Command);
}

#[test]
fn in_moves_the_place_to_pattern() {
    assert_eq!(after_word(b"in", Place::Command), Place::Pattern);
    assert_eq!(after_word(b"in", Place::Data), Place::Pattern);
}

/// The glued cond pair is *two* word starts (task #236's S2): the byte-level
/// scans break words at operator bytes, so a cond pair meets them twice, and
/// a *tail word* (`|if`, `&if`) is one of those starts. The rows pinned here
/// are the pipeline-guard and operator-form rows — behind data every
/// operator tail is an argument (#292's carve-out: a block written as a
/// pipeline stage), a pipeline tail keeps the `case` word list, a redirect
/// tail (`&>` in `x&>f`) is an argument, and a word that *contains* a pair
/// (`p||if`, no glue: the scans cut words at the operator bytes) is an
/// argument.
#[test]
fn operator_pair_halves_keep_the_place_the_pair_set() {
    // A pipeline tail behind data is an argument (#292's carve-out row) and
    // behind a `case` word list it keeps the list.
    assert_eq!(after_word(b"|if", Place::Data), Place::Data);
    assert_eq!(after_word(b"|if", Place::Pattern), Place::Data);
    // A redirect tail's payload is data.
    assert_eq!(after_word(b"&>", Place::Command), Place::Data);
    // A word that *contains* a pair classifies as an argument.
    assert_eq!(after_word(b"p||if", Place::Data), Place::Data);
    assert_eq!(after_word(b"p|if", Place::Data), Place::Data);
    // The pair-form halves are command starters for the word behind them.
    for word in [b"&&".as_slice(), b"||", b"&", b";"] {
        assert_eq!(
            after_word(word, Place::Data),
            Place::Command,
            "the operator {word:?} starts a command"
        );
    }
}

/// The cond pair's *payload* arms (task #236's S2): a pair's head word
/// (`||if`) and tail word (`|if`) carry the keyword behind their operator
/// bytes, and that keyword takes the place the pair sets — every
/// condition-taking opener and every clause keyword opens a command, `in`
/// opens a `case`/`for` word list. Behind **data** the payload stays data:
/// that is the in-block glued divergence the README bullet pins and
/// `tests/glued_pair.rs` measures (`echo a&&if` is an argument). A bare glue
/// (an empty payload) leaves the place the pair sets; an operator-shaped
/// payload is a redirect form, an argument; a `time [-p]` payload is
/// transparent, keeping the place.
#[test]
fn pair_payloads_take_the_place_the_pair_sets() {
    for (head, tail) in [
        (b"||if".as_slice(), b"|if".as_slice()),
        (b"&&while", b"&while"),
        (b"||until", b"|until"),
        (b"&&then", b"&then"),
        (b"||do", b"|do"),
        (b"&&else", b"&else"),
        (b"||elif", b"|elif"),
    ] {
        for (word, glue) in [(head, Glue::Head), (tail, Glue::Tail)] {
            assert_eq!(
                after_word_glue(word, Place::Command, glue),
                Place::Command,
                "the pair {word:?} opens a command"
            );
            assert_eq!(
                after_word_glue(word, Place::Data, glue),
                Place::Data,
                "the pair {word:?} behind data is an argument"
            );
            assert_eq!(
                after_word_glue(word, Place::Pattern, glue),
                Place::Command,
                "the pair {word:?} ends the word list it sits in"
            );
        }
    }
    // `in` opens the word list from both halves.
    for (word, glue) in [
        (b"&&in".as_slice(), Glue::Head),
        (b"|in".as_slice(), Glue::Tail),
    ] {
        assert_eq!(
            after_word_glue(word, Place::Command, glue),
            Place::Pattern,
            "{word:?} opens the list"
        );
    }
    // A bare glue (no payload) leaves the place the pair set: the head opens
    // a command, the tail is transparent.
    assert_eq!(
        after_word_glue(b"||", Place::Data, Glue::Head),
        Place::Command
    );
    assert_eq!(
        after_word_glue(b"|", Place::Command, Glue::Tail),
        Place::Command
    );
    assert_eq!(after_word_glue(b"|", Place::Data, Glue::Tail), Place::Data);
    // An operator-shaped payload is a redirect form, an argument.
    assert_eq!(
        after_word_glue(b"||||", Place::Command, Glue::Head),
        Place::Data
    );
    // A `time [-p]` payload is transparent: it keeps the pair's place.
    assert_eq!(
        after_word_glue(b"||time", Place::Command, Glue::Head),
        Place::Command
    );
    assert_eq!(
        after_word_glue(b"||time -p", Place::Data, Glue::Head),
        Place::Data
    );
}

/// Ordinary words, closers, `for`/`case` (their lists/conditions are read by
/// the `in` and clause rules, not by them) and the pipeline `|` (a block
/// written as a pipeline stage stays word data, task #292) are arguments.
#[test]
fn argument_words_move_the_place_to_data() {
    for word in [
        b"echo".as_slice(),
        b"x",
        b"fi",
        b"done",
        b"esac",
        b"for",
        b"case",
        b"|",
        b">",
        b"&>",
        b"'q'",
        b"#",
        b"whileish",
    ] {
        assert_eq!(
            after_word(word, Place::Command),
            Place::Data,
            "{word:?} is an argument word"
        );
    }
}

/// A word that is only a `time [-p]` prefix is a statement prefix, not a word
/// of its own: it keeps the current place (a keyword behind it is scored on
/// the rest of the word by `keyword_delta` itself).
#[test]
fn time_prefix_is_transparent() {
    for word in [b"time".as_slice(), b"time -p", b"time time -p"] {
        assert_eq!(after_word(word, Place::Data), Place::Data);
        assert_eq!(after_word(word, Place::Pattern), Place::Pattern);
        assert_eq!(after_word(word, Place::Command), Place::Command);
    }
    // Only a whole-prefix word is transparent; `time` behind data is data.
    assert_eq!(after_word(b"mtime", Place::Command), Place::Data);
}

/// The `time` transparency applies to the flag *and* to the score
/// (`keyword_delta` strips the prefix itself): the keyword behind a prefix
/// sees the prefix's place.
#[test]
fn keyword_behind_a_time_prefix_keeps_the_place() {
    let line = b"time if";
    let word_end = first_word_end(line, 0);
    assert_eq!(
        after(line, line, word_end, Place::Pattern, Glue::None),
        Place::Command
    );
    assert!(scores(b"time if", b"time if", 7, Place::Command));
    assert!(!scores(b"time if", b"time if", 7, Place::Data));
}

#[test]
fn openers_score_in_command_position_only() {
    for word in [b"if".as_slice(), b"while", b"until", b"for", b"case"] {
        assert!(scores_word(word, Place::Command), "{word:?} opens");
        assert!(
            !scores_word(word, Place::Pattern),
            "{word:?} in a pattern list is a pattern word"
        );
        assert!(
            !scores_word(word, Place::Data),
            "{word:?} in an argument is data"
        );
    }
}

/// A closer scores in command position *and* in a `case` list — the list's
/// terminator is a closer, and the gate is a *place* rule, not a per-word one
/// (`case x in esac` is a complete `case`; a bare closer written as a pattern
/// word is the pattern-list divergence) — but never in an argument: bash
/// requires a separator before a closing keyword, so the opener gate
/// (Command only) and the closer gate (Command | Pattern) are not one gate.
#[test]
fn closers_score_in_command_and_pattern_positions() {
    for word in [b"fi".as_slice(), b"done", b"esac"] {
        assert!(scores_word(word, Place::Command), "{word:?} closes");
        assert!(
            scores_word(word, Place::Pattern),
            "a closer at a pattern list scores ({word:?})"
        );
        assert!(
            !scores_word(word, Place::Data),
            "{word:?} in an argument is data"
        );
    }
}

/// The brace carve-out (task #236 §5): `{`/`}` stay ungated in every place,
/// the divergence `tests/group.rs::literal_brace_argument_in_a_block_body`
/// pins. `case x in if)` stays a pattern: the opener gate covers keywords,
/// not braces.
#[test]
fn brace_words_are_ungated() {
    for place in [Place::Command, Place::Pattern, Place::Data] {
        assert!(
            scores(b"{ ", b"{ ", 1, place),
            "a group opener scores at {place:?}"
        );
        assert!(
            scores(b"}", b"}", 1, place),
            "a literal closer scores at {place:?}"
        );
    }
}

#[test]
fn non_keyword_words_never_score() {
    for word in [
        b"echo".as_slice(),
        b"do",
        b"then",
        b"else",
        b"in",
        b"true",
        b"ifconfig",
        b"donec",
        b"fi)",
        b"esac)",
    ] {
        assert!(
            !scores_word(word, Place::Command),
            "{word:?} is not a block keyword"
        );
    }
}

/// A `wait` scores only as the event-case block opener (a pattern keyword
/// behind it); the POSIX builtin is word data in every place.
#[test]
fn builtin_wait_never_scores_and_event_case_only_in_command() {
    let builtin = b"wait 123";
    let event = b"wait\n readable %rd)";
    for place in [Place::Command, Place::Pattern, Place::Data] {
        assert!(
            !scores(builtin, builtin, 8, place),
            "the builtin is data at {place:?}"
        );
    }
    assert!(
        scores(event, event, 4, Place::Command),
        "the event-case `wait` opens"
    );
    assert!(
        !scores(event, event, 4, Place::Data),
        "the event-case `wait` in an argument is data"
    );
}
