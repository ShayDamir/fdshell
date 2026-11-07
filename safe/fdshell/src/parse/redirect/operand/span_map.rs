//! The span↔token offset map of `parse/redirect/operand.rs` (task #458). The
//! arms answer "where does the operand begin" in **token text** (quotes
//! stripped); the pre-parse rewriter slices the token's **raw span** (quotes
//! present), so the answer must move past every quote byte the tokenizer
//! stripped before it. The decode uses only the tokenizer's own recorded facts
//! — one mask region per non-empty quote pair (`parse/token/mask.rs`) and one
//! `spans` anchor per zero-byte pair (task #321), pinned by
//! `parse/token/tests.rs` — no byte rescan, no second copy of the tokenizer's
//! rules. Exact on the reachable domain: the arms accept a quote only in the
//! io-number digits, so every region before the offset is a real pair, and the
//! mask's escape and substitution subtleties (an escape inside a `"` region
//! pushes extra stripped bytes, a top-level substitution region marks quoted
//! but strips nothing) can only hide in prefix bytes that are not digits —
//! the parser's own io-number gate rejects those before any mapping happens
//! (e.g. `"2\"3\">…` → prefix text `2"3` → no operand).

use crate::mask::MaskBit;

/// Map the token-text offset `off` into the token's raw span. Every non-empty
/// mask region that **ends strictly before** `off` contributes its pair's two
/// delimiters; every `spans` anchor **at or before** `off` contributes its
/// pair's two bytes (the pair sits between text bytes `j - 1` and `j`, hence
/// before text byte `off`). A region that reaches `off` holds the operand's own
/// first bytes, so its delimiters stay inside the region — the map is the
/// identity there, and the message keeps naming the operand as written, quotes
/// included. An offset of `0` has nothing before it, so it never moves.
pub(super) fn span_offset(mask: &[MaskBit], spans: &[usize], off: usize) -> usize {
    if off == 0 {
        return 0;
    }
    let mut raw = off;
    for (end, (&bit, &next)) in mask.iter().zip(mask.iter().skip(1)).enumerate() {
        if bit.quoted() && !matches!(next, MaskBit::Quoted) && end < off {
            raw += 2;
        }
    }
    for j in spans.iter().copied() {
        if j <= off {
            raw += 2;
        }
    }
    raw
}
