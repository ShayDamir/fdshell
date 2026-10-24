//! Unit pins for the shared command-position rule: every [`Place`] transition
//! of [`after`], every gate arm of [`scores`] (the two word-list places of
//! [`list`] included), and the asymmetries the rule encodes (opener vs closer,
//! `case` list vs `for` list vs argument, brace carve-out, `time [-p]`
//! transparency).

use super::list;
use super::{Glue, Place, after, body_keyword, clause_keyword, list_keyword, list_opener, scores};
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
    for word in [b"".as_slice(), b";", b"&&", b"||", b"&", b"(", b")", b"{ "] {
        assert_eq!(
            after_word(word, Place::Data),
            Place::Command,
            "the word behind {word:?} starts a command"
        );
        assert_eq!(after_word(word, Place::Pattern), Place::Command);
    }
}

/// The clause keywords (`if`/`while`/`until` and `then`/`do`/`else`/`elif`)
/// restart command position **only where they are not an argument** (task
/// #301): `echo a else fi` writes `else` as `echo`'s word, and from there `fi`
/// must stay an argument too — the closer-shaped word that follows an argument
/// keyword is word data (bash rc 2 for that form, measured). A `case` word list
/// (`Place::Pattern`) keeps the old row: the list's `)` ends it, and its glued
/// pair words are pinned in [`pair_payloads_take_the_place_the_pair_sets`].
#[test]
fn clause_keywords_restart_command_position_only_from_command_place() {
    for word in [
        b"if".as_slice(),
        b"while",
        b"until",
        b"then",
        b"else",
        b"elif",
        b"do",
    ] {
        assert_eq!(
            after_word(word, Place::Command),
            Place::Command,
            "{word:?} in command position opens a command"
        );
        assert_eq!(
            after_word(word, Place::Pattern),
            Place::Command,
            "{word:?} ends the word list it sits in"
        );
        assert_eq!(
            after_word(word, Place::Data),
            Place::Data,
            "{word:?} as an argument keeps the argument place"
        );
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

/// A `for` in **command position** opens its own **word list**
/// ([`Place::Words`], task #306): the opener, not `in`, is the word that tells
/// the two list kinds apart. The row is command-position-only — in a `case`
/// pattern list `for` is a pattern word, behind data it is an argument — and it
/// reads the whole word, so a quoted `"for"`, a `forx`-shaped word and the
/// arithmetic `for ((…))` (which reaches the row as one glued word) stay out.
#[test]
fn a_for_in_command_position_opens_a_word_list() {
    assert_eq!(
        after_word(b"for", Place::Command),
        Place::Words,
        "`for` in command position opens a word list"
    );
    assert_eq!(
        after_word(b"for", Place::Pattern),
        Place::Data,
        "`for` in a `case` pattern list is a pattern word"
    );
    assert_eq!(
        after_word(b"for", Place::Data),
        Place::Data,
        "`for` as an argument is data"
    );
    assert_eq!(
        after_word(b"\"for\"", Place::Command),
        Place::Data,
        "a quoted `for` is a word"
    );
    assert_eq!(
        after_word(b"foreish", Place::Command),
        Place::Data,
        "`foreish` is no keyword"
    );
    // The arithmetic `for ((…))` reaches the row as the one glued word
    // `for((…))`, so it opens no list place; the word's own trailing `)` is the
    // `case`-clause-`)` row of [`glued_case_clause_close_moves_the_place_to_command`],
    // which it keeps.
    assert_eq!(
        after_word(b"for((i=0;i<2;i++))", Place::Command),
        Place::Command,
        "the arithmetic `for ((…))` opens no list place"
    );
}

/// The list place is **sticky**: every keyword-shaped word of a `for` list
/// keeps it, and only the separator that ends the list (the `;`/newline of its
/// run, the `&&`/`||` of the accepted divergence, a substitution's paren) or
/// the `do` that opens the body leaves it (task #306). One assertion per arm of
/// [`list::step`], so each arm is a killable mutant.
#[test]
fn a_for_list_keeps_its_place_for_every_word() {
    for word in [
        b"done".as_slice(),
        b"fi",
        b"esac",
        b"then",
        b"else",
        b"echo",
        b"{ ",
        b"}",
        b"in",
        b"a&&b",
    ] {
        assert_eq!(
            after_word(word, Place::Words),
            Place::Words,
            "the list word {word:?} keeps the list"
        );
    }
    for word in [b"do".as_slice(), b";", b"&&", b"||", b"&", b"(", b")", b""] {
        assert_eq!(
            after_word(word, Place::Words),
            Place::Command,
            "{word:?} leaves the list"
        );
    }
    // A cond pair's head/tail word is no arm: the accepted `for i in a && b`
    // divergence rides the sticky place, and a `do` glued behind a pair is a
    // pair payload, judged by the pair's rule, not by the list's.
    for glue in [Glue::Head, Glue::Tail] {
        assert_eq!(
            after_word_glue(b"&&", Place::Words, glue),
            Place::Words,
            "a glued pair keeps the list ({glue:?})"
        );
        assert_eq!(
            after_word_glue(b"do", Place::Words, glue),
            Place::Words,
            "a glued `do` is no list terminator ({glue:?})"
        );
    }
}

/// The gate at a `for` word list scores **nothing**: no opener, no closer, no
/// brace word, not the event-case `wait` (bash runs `for i in done`,
/// `for i in esac`, `for i in { 1 2` and `for i in }` as words — task #306).
/// The gate is the *place*, which is why the `case` list keeps its opposite
/// rule (`case x in esac` is a complete `case`) and why the list gate runs
/// before the brace carve-out of [`brace_words_are_ungated`].
#[test]
fn nothing_scores_at_a_for_word_list() {
    for word in [
        b"for".as_slice(),
        b"if",
        b"while",
        b"done",
        b"fi",
        b"esac",
        b"{ ",
        b"}",
        b"wait",
    ] {
        assert!(
            !scores_word(word, Place::Words),
            "the list word {word:?} never scores"
        );
    }
    // The place, not the word: the event-case `wait` (a pattern keyword behind
    // it) opens a block in command position and is word data in a list.
    let event = b"wait\n readable %rd)";
    assert!(scores(event, event, 4, Place::Command));
    assert!(!scores(event, event, 4, Place::Words));
    // The two list places are the asymmetry: the same closer scores at the
    // `case` list, never at a `for` list.
    assert!(scores_word(b"esac", Place::Pattern));
    assert!(!scores_word(b"esac", Place::Words));
    // [`list::scores`] gates the list place and nothing else.
    assert!(!list::scores(Place::Words));
    for place in [Place::Command, Place::Pattern, Place::Data] {
        assert!(
            list::scores(place),
            "{place:?} is not gated by the list rule"
        );
    }
}

/// The keyword rows `after` reads are **one set** with the cond-boundary scan's
/// clause/body mode stack (`cond/boundary/block/clause`): each predicate answers
/// exactly the words of its own arm, and every other word — including a `time`
/// prefix and a glued payload — answers none of them.
#[test]
fn the_clause_keyword_set_is_one_set_with_after() {
    for word in [b"then".as_slice(), b"else", b"do"] {
        assert!(body_keyword(word), "{word:?} opens a body");
        assert!(!clause_keyword(word), "{word:?} opens no clause");
        assert!(!list_keyword(word), "{word:?} opens no word list");
        assert!(!list_opener(word), "{word:?} opens no list-owning block");
        assert_eq!(
            after_word(word, Place::Command),
            Place::Command,
            "`after` reads {word:?} through the same row"
        );
    }
    assert!(clause_keyword(b"elif"), "`elif` opens a clause");
    assert!(!body_keyword(b"elif"), "`elif` opens no body");
    assert_eq!(after_word(b"elif", Place::Command), Place::Command);
    assert!(list_keyword(b"in"), "`in` opens a word list");
    assert!(!body_keyword(b"in"), "`in` opens no body");
    assert_eq!(after_word(b"in", Place::Command), Place::Pattern);
    assert!(list_opener(b"for"), "`for` owns a word list");
    assert!(!list_keyword(b"for"), "`for` is no list keyword");
    assert_eq!(
        after_word(b"for", Place::Command),
        Place::Words,
        "`after` reads `for` through the same row"
    );
    // None of the four reads anything else: the openers and closers are
    // `keyword_delta`'s rows, and a word that merely *starts* like a keyword
    // stays out of the set.
    for word in [
        b"if".as_slice(),
        b"while",
        b"until",
        b"fi",
        b"done",
        b"esac",
        b"x)",
        b"thenx",
        b"int",
        b"elifx",
        b"time then",
        b"forx",
        b"for((i=0;i<2;i++))",
    ] {
        assert!(!body_keyword(word), "{word:?} is no body keyword");
        assert!(!clause_keyword(word), "{word:?} is no clause keyword");
        assert!(!list_keyword(word), "{word:?} is no word-list keyword");
        assert!(!list_opener(word), "{word:?} opens no list-owning block");
    }
}

/// The glued cond pair is *two* word starts (task #236's S2): the byte-level
/// scans break words at operator bytes, so a cond pair meets them twice, and
/// a *tail word* (`|if`, `&if`) is one of those starts. The rows pinned here
/// are the pipeline-guard and operator-form rows — behind data a *glued*
/// operator tail is an argument, a pipeline tail keeps the `case` word list
/// (its `|` is the pattern alternation), a redirect tail (`&>` in `x&>f`) is an
/// argument, and a word that *contains* a pair (`p||if`, no glue: the scans cut
/// words at the operator bytes) is an argument.
#[test]
fn operator_pair_halves_keep_the_place_the_pair_set() {
    // A pipeline tail behind a `case` word list keeps the list (task #292's
    // one guard: `case x in a|while)` stays a pattern).
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
    // A payload that is only a pipeline `|` is an **operator run**, so the pair
    // behind it is data from every place, command position included: the arm's
    // data half alone does not cover this shape, and the fall-through behind it
    // is #292's stage rule, which would hand command position to the keyword
    // written there (`true &&| if …`).
    for (word, glue) in [
        (b"&&|".as_slice(), Glue::Head),
        (b"&|".as_slice(), Glue::Tail),
    ] {
        assert_eq!(
            after_word_glue(word, Place::Command, glue),
            Place::Data,
            "the pipeline payload {word:?} never starts a command"
        );
        assert_eq!(
            after_word_glue(word, Place::Pattern, glue),
            Place::Data,
            "…and it does not end a word list either"
        );
    }
    // A payload that only *looks* like an opener is an argument: each arm's
    // guard is the whole payload, not its first letter.
    assert_eq!(
        after_word_glue(b"|usr", Place::Command, Glue::Tail),
        Place::Data
    );
    // A payload glued to a pattern's closing `)` starts the arm's body: the
    // word is judged by its last byte (`case x in a)|echo b`).
    assert_eq!(
        after_word_glue(b"||x)", Place::Command, Glue::Head),
        Place::Command
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

/// Ordinary words, `case` (its lists/conditions are read by the `in` and
/// clause rules, not by it) are arguments. `for` is **not** in this list: in
/// command position it owns a word list, pinned by
/// [`a_for_in_command_position_opens_a_word_list`]. The closers are *not* in
/// this list either:
/// [`a_closer_in_command_position_restarts_command_position`] pins their own
/// row.
#[test]
fn argument_words_move_the_place_to_data() {
    for word in [
        b"echo".as_slice(),
        b"x",
        b"case",
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

/// A **closer** in command position ends the compound command it closes, and
/// bash's grammar makes the `;` after a compound command optional, so the word
/// behind it starts a command again (task #301:
/// `while :; do if :; then :; fi done`, `if :; then { echo a; } fi` are rc 0 in
/// bash). Behind data the closer-shaped word is an argument and leaves the place
/// alone (`echo fi`), and a quoted `"fi"` word is data at both places
/// (task #247's gate, inside `keyword_delta` itself).
#[test]
fn a_closer_in_command_position_restarts_command_position() {
    for word in [b"fi".as_slice(), b"done", b"esac", b"}"] {
        assert_eq!(
            after_word(word, Place::Command),
            Place::Command,
            "the closer {word:?} restarts command position"
        );
        assert_eq!(
            after_word(word, Place::Data),
            Place::Data,
            "the closer-shaped word {word:?} as an argument stays data"
        );
    }
    // The subshell's `)` is a separator token, so it starts a command from
    // every place already ([`command_starters_move_the_place_to_command`]).
    // A glued closer (`}>f`, `fi>f`) is a closer in `keyword_delta` too, so it
    // restarts command position; the builder-side predicate does **not** accept
    // it as a closer predecessor (`parse/semi/preceded/tests.rs`).
    assert_eq!(after_word(b"}>f", Place::Command), Place::Command);
    assert_eq!(after_word(b"esac>f", Place::Command), Place::Command);
    // A quoted closer is a word: `echo "fi"` leaves the place alone.
    assert_eq!(after_word(b"\"fi\"", Place::Command), Place::Data);
    assert_eq!(after_word(b"\"fi\"", Place::Data), Place::Data);
    // A `case` word list is not the compound command the rule reads: the list's
    // own `)`/`esac` rules end it, so a closer there leaves data (the list rows
    // above and in [`closers_score_in_command_and_pattern_positions`]).
    assert_eq!(after_word(b"fi", Place::Pattern), Place::Data);
    // The arm's gate is a **negative** delta, not a non-positive one: the POSIX
    // builtin `wait` scores `Some(0)` (`keywords::keyword_delta` returns the
    // event-case opener's flag), and a zero is not a compound command's end.
    // End-to-end: `while false; do wait done` is rc 1 `'while' is missing its
    // closing 'done'` (`tests/block_closers.rs`), not `expected ';'`.
    assert_eq!(after_word(b"wait", Place::Command), Place::Data);
    assert_eq!(after_word(b"wait", Place::Pattern), Place::Data);
}

/// The pipeline tail is a **command** starter (task #292): the stage behind a
/// `|` is a command, so a keyword glued to the pipe byte scores, at top level
/// and inside a block alike. A `||` spelled with no glue is not a tail (the
/// cond-pair rows pin it), and a word that merely *ends* with the pipe byte is
/// judged by its own first byte.
#[test]
fn the_pipeline_tail_starts_a_command() {
    for word in [b"|".as_slice(), b"|if", b"|while", b"| until"] {
        assert_eq!(
            after_word(word, Place::Data),
            Place::Command,
            "the pipeline tail {word:?} opens a stage"
        );
        assert_eq!(after_word(word, Place::Command), Place::Command);
    }
    assert_eq!(after_word(b"||if", Place::Data), Place::Data);
    assert_eq!(after_word(b"p|", Place::Data), Place::Data);
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
            !scores_word(word, Place::Words),
            "{word:?} in a `for` list is a list word"
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
/// A `for` list gates the closer outright, the opposite list kind
/// ([`nothing_scores_at_a_for_word_list`]).
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
    // The `for` **list** gates braces (task #306): bash reads `for i in { 1 2`
    // and `for i in }` as words. The list gate runs first, so this body
    // divergence stops at the list's edge.
    assert!(!scores(b"{ ", b"{ ", 1, Place::Words));
    assert!(!scores(b"}", b"}", 1, Place::Words));
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
    for place in [Place::Command, Place::Pattern, Place::Words, Place::Data] {
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
    assert!(
        !scores(event, event, 4, Place::Words),
        "the event-case `wait` in a `for` list is a list word"
    );
}
