//! The glued-`)` **closer** payload of a word (task #421): the closer candidate
//! behind the `)` that ends a `case` pattern word — `a)esac` → `esac`,
//! `b)esac>f` → `esac>f` (the operand behind the glue may be quoted, task #413).
//! A keyword block's closing keyword is the one place the glue is read the way
//! bash reads it (README, the `esac>f` rule); here the glue byte in front of the
//! closer is the pattern's `)`, a byte of the **one word-break set**
//! ([`crate::bytes::word_break`]) that ends the tokenizer's keyword word too.
//!
//! The consumer is the byte-run scorer [`crate::comment::depth_delta`], which is
//! the only scorer whose word unit differs from the tokenizer's: it splits a run
//! at spaces, so `b)esac` arrives as one word and the word-start rows of
//! [`crate::keywords::keyword_kind`] see no keyword in it. This is its sibling of
//! [`crate::keywords::pair_payload`] (task #236's S2), and it is scored at the
//! word's own place through the same gates — the place gate, the **kind** gate
//! and the stray-closer clamp keep firing; only the payload is read.
//!
//! `None` when the word carries no `)`, when nothing sits behind it, and when the
//! `)` is **region bytes** of an arithmetic word (task #396's carve-out: the
//! [`list_opener`] head, the `(( … ))` region itself). It answers **closers
//! only** — an opener glued behind a `)` (`case x in a)if :; then :; fi;; esac`)
//! stays open (task #316's family), so this predicate never reads one. One score
//! per word is the call site's: the word-start row owns a word it recognizes, and
//! the payload is scored for a word the rows leave word data.

use crate::keywords::list_opener;

/// The arithmetic word whose `)` are **region bytes**, never the clause close:
/// the `(( … ))` region written as a word of its own, and the `for` head glued to
/// it (task #396 — the row of [`crate::keywords::position::after`] that keeps
/// `for((…))` out of the glued-`)` rule).
fn region_word(word: &[u8]) -> bool {
    word.starts_with(b"((") || list_opener(word)
}

/// The closer candidate behind the word's first `)` byte: `a)esac` → `esac`,
/// `b)esac>f` → `esac>f`, `b)esac>f"f"` → `esac>f"f"`. `None` for a word with no
/// `)` byte, for a `)` that ends the word (the plain glued pattern `a)` — the
/// scorer's word-start rows own it), for an empty payload, and for an arithmetic
/// region word. A quoted payload (`a)"esac"`) is returned as the raw bytes it is:
/// a single-quoted word never reaches the scorer at all, and a quoted payload is
/// answered by [`crate::keywords::keyword_kind`]'s quote gate, which reads the
/// quotes as the operand's, never the keyword's (task #413).
pub(crate) fn closer_payload(word: &[u8]) -> Option<&[u8]> {
    if region_word(word) {
        return None;
    }
    let at = word.iter().position(|&b| b == b')')?;
    let payload = word.get(at + 1..)?;
    (!payload.is_empty()).then_some(payload)
}

#[cfg(test)]
mod tests;
