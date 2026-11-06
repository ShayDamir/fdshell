//! One word of the run scored through the keyword gates — task #236's
//! command-position rule, moved out of `depth_delta.rs`.

use crate::keywords::{Glue, Pairing, Place, after, closer_payload, pair_payload, scores};

/// Score one word of the run and move the command-position flag with it
/// (task #236): bash recognizes a block keyword in command position *only*,
/// so a keyword-shaped *argument* (`echo if`, `echo done`) is word data, and
/// only the words behind a run start, a cond operator, `then`/`do`/`else`/
/// `elif`, a condition-taking opener, a group opener or a `case` clause's `)`
/// can open or close a nested block. A brace word stays ungated, the accepted
/// divergence `tests/group.rs::literal_brace_argument_in_a_block_body` pins.
///
/// The score is [`Pairing::apply`]'s (task #309): an opener pushes its own kind,
/// a closer pops only the block it belongs to, and a closer of another kind
/// scores `0` — the sum is the run's depth delta, its kind pairing is what the
/// scan of a block span needs.
///
/// The pairing's [`Pairing::observe`] carries task #436's record: the word's own
/// start offset, once per scan (a `time [-p]` prefix written in front of the
/// closer belongs to the same word run, and [`crate::error::parse`] resolves the
/// keyword behind it for the caret and the message),
/// for a closer word in **command position** with **nothing open** — the scan's
/// top level, where bash's grammar has no command that is a lone closing keyword.
/// The gate is the empty stack, not the mismatch arm of [`Pairing`]: a closer
/// that mismatches the innermost open block only says the *scanner* did not pair
/// it, and its score-0 answer is load-bearing exactly there — a glued pipeline
/// stage, a cond-pair word and a `case` word slot all hide an opener bash sees
/// (`tests/cond_blocks.rs::block_stage_in_a_loop_body_ends_the_loop`,
/// `short_circuit_gate_skips_the_gated_list_and_resumes_behind_it`,
/// `a_closer_shaped_case_pattern_list_word_is_a_word`, all bash rc 0).
/// The score is unchanged by the decision, and only the top-level scan passes a
/// sink that is not `None` — the nested scorers walk text that never runs, and a
/// scan records the word once (`comment/tests.rs`, `keywords/pairing/tests.rs`).
pub(crate) fn word_step(
    frag: &[u8],
    line: &[u8],
    word_end: usize,
    place: &mut Place,
    pairing: &mut Pairing,
) -> i32 {
    // The closer row of [`after`] is gated by the pairing's innermost block,
    // read **before** this word is applied: a closer pairs with the block it
    // closes (task #309).
    let top = pairing.top();
    // The **decision** of task #436 is read at the same moment, for a word the
    // place gate puts in command position — a `case` word list keeps its
    // closer-shaped words, and an argument word never asks.
    pairing.observe(frag, line, word_end, *place);
    let delta = if scores(frag, line, word_end, *place) {
        pairing.apply(frag, line, word_end)
    } else {
        0
    };
    // The glued-pair payload (task #236's S2): a fragment carrying a cond
    // pair (`echo a&&if` is one whitespace unit) scores the keyword behind
    // the pair at the fragment's place — `echo a&&if` stays data (argument),
    // `a&&if` at a clause start nests the `if` so its `fi` closes *the if*.
    // The payload is scored at the *fragment's* end in `line` (`word_end`):
    // the payload is a slice of the fragment, so the fragment's own end is
    // the only offset into `line` that is true for it.
    let payload_delta = pair_payload(frag, &[]).map_or(0, |payload| {
        if scores(payload, line, word_end, *place) {
            pairing.apply(payload, line, word_end)
        } else {
            0
        }
    });
    // The glued-`)` closer payload (task #421): the closer behind a `case`
    // pattern word's `)` (`b)esac`, `b)esac>f`) — a block's closing keyword is
    // the one place the glue is read the way bash reads it, and this run scorer
    // is the one whose word unit is coarser than the tokenizer's. It is scored
    // at the fragment's place through the same gates as the cond-pair payload
    // above, so the place gate, the kind gate and the stray-closer clamp keep
    // firing. **One score per word:** the word-start row owns a word it
    // recognizes, so the payload is read only for a word that row leaves word
    // data — what pins `case x in esac) …` as the word-list rule's.
    let closer_delta = match (delta, closer_payload(frag)) {
        (0, Some(payload)) if scores(payload, line, word_end, *place) => {
            pairing.apply(payload, line, word_end)
        }
        _ => 0,
    };
    // The fragment is the run's *whitespace* unit: a glued pair is one
    // fragment, never a bare tail byte, so the fragment is never in a pair
    // ([`Glue::None`]).
    *place = after(frag, line, word_end, *place, Glue::None, top, &[]);
    delta + payload_delta + closer_delta
}
