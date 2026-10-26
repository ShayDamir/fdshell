//! The state of one word's substitution: the byte stream, the output text with
//! its parallel mask, the byte provenance and the word index, plus the inputs
//! that stay fixed for the whole run. `arg.rs` builds it and loops over the
//! bytes; every arm in `arg/arms.rs` moves it through [`Ex::parts`].

use alloc::vec::Vec;
use hashbrown::HashMap;
use sys::ExportedFd;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use crate::mask::MaskBit;
use crate::state::ShellState;
use crate::substitute::Join;
use crate::substitute::mask::Counting;
use crate::substitute::origin::Origin;

/// The state of one word's substitution, shared by every arm of the loop:
/// `peek` hands out the word's bytes and counts them, `idx` is the word index
/// the input `mask` is read at, and `out`/`out_mask`/`origin` accumulate the
/// output. `mask`, `join`, `cache` and `cell` are the run's inputs, fixed for
/// the whole word: carrying them here keeps each arm's signature down to the
/// arguments that arm alone moves.
pub(crate) struct Ex<'a> {
    pub peek: Counting<'a>,
    pub idx: usize,
    pub out: ShortCStr,
    pub out_mask: Vec<MaskBit>,
    pub origin: Origin,
    pub cell: &'a ForkCell<ShellState>,
    pub cache: &'a mut HashMap<ShortCStr, ExportedFd>,
    pub mask: &'a [MaskBit],
    pub join: Join,
}

/// `Ex`'s fields as borrows, in field order: the eight pieces an arm moves,
/// under the short names the substitution loop spells them by. `cell` is a
/// **copy** of the stored shared reference, so the borrow an arm takes through
/// it is not a borrow of the state.
pub(super) type Parts<'x, 'a> = (
    &'x mut Counting<'a>,
    &'x mut usize,
    &'x mut ShortCStr,
    &'x mut Vec<MaskBit>,
    &'x mut Origin,
    &'a ForkCell<ShellState>,
    &'x mut HashMap<ShortCStr, ExportedFd>,
    Join,
);

impl<'a> Ex<'a> {
    /// Borrow (and copy, for `cell`/`join`) the state an arm moves.
    pub(super) fn parts<'x>(&'x mut self) -> Parts<'x, 'a> {
        (
            &mut self.peek,
            &mut self.idx,
            &mut self.out,
            &mut self.out_mask,
            &mut self.origin,
            self.cell,
            self.cache,
            self.join,
        )
    }
}
