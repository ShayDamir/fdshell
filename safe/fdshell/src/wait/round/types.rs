//! The types of one poll round: what a polled fd is released as, the
//! descriptors to poll, and the round's `after` arms and deadline.

use alloc::vec::Vec;

use sys::ShortCStr;

/// What to release (close) a polled fd as when its arm does not keep it.
pub(crate) enum ReleaseKey {
    None,
    Var(ShortCStr),
    Array { arr: ShortCStr, source: ShortCStr },
    Task(ShortCStr),
}

/// One descriptor to poll, tied to the arm that requested it.
pub(crate) struct PollEntry {
    pub raw: i32,
    pub events: i16,
    pub revents: i16,
    pub ready_mask: i16,
    pub arm: usize,
    pub release: ReleaseKey,
    pub finished: bool,
}

impl PollEntry {
    /// A not-yet-polled entry: `revents` is zero until [`sys::poll`] fills it.
    pub(super) fn new(
        raw: i32,
        events: i16,
        ready_mask: i16,
        arm: usize,
        release: ReleaseKey,
        finished: bool,
    ) -> Self {
        Self {
            raw,
            events,
            revents: 0,
            ready_mask,
            arm,
            release,
            finished,
        }
    }
}

/// The resolved descriptors, `after` arms, and deadline of one poll round.
pub(crate) struct Round {
    pub entries: Vec<PollEntry>,
    pub after_arms: Vec<usize>,
    pub timeout: i32,
}
