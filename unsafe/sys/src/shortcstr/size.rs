#[derive(Clone, Copy)]
#[repr(u8)]
pub enum InlineSize {
    _0,
    _1,
    _2,
    _3,
    _4,
    _5,
    _6,
    _7,
    _8,
    _9,
    _10,
    _11,
    _12,
    _13,
    _14,
    _15,
    _16,
    _17,
    _18,
    _19,
    _20,
    _21,
    _22,
    _23,
    _24,
    _25,
    _26,
    _27,
    _28,
    _29,
    _30,
}

impl InlineSize {
    /// # Safety
    /// `v` must be ≤ `INLINE_MAX`.
    pub(crate) unsafe fn from_u8(v: u8) -> Self {
        debug_assert!(v <= crate::shortcstr::INLINE_MAX);
        // SAFETY: caller guarantees `v ≤ INLINE_MAX`, which selects
        // a valid discriminant of the `#[repr(u8)]` enum.
        unsafe { core::mem::transmute(v) }
    }

    pub(crate) fn as_u8(self) -> u8 {
        self as u8
    }
}
