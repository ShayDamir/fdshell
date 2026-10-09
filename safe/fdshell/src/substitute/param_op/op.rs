//! The operator of a `${name<op>word}` braced expansion: the colon family takes
//! a word, the pattern family a word pattern (`*`, `?`, `[...]`, `\X`, quoted
//! bytes) matched against the parameter's value.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::substitute) enum ParamOp {
    /// `:-` — the word when the parameter is unset or empty.
    Default,
    /// `:=` — the word, assigned to the parameter.
    Assign,
    /// `:+` — the word when the parameter is set and non-empty.
    Alternate,
    /// `:?` — bail with the word as the message.
    Error,
    /// `#` — strip the shortest matching prefix.
    ShortestPrefix,
    /// `##` — strip the longest matching prefix.
    LongestPrefix,
    /// `%` — strip the shortest matching suffix.
    ShortestSuffix,
    /// `%%` — strip the longest matching suffix.
    LongestSuffix,
}

impl ParamOp {
    /// The operator's byte count, which locates the pattern's mask slice.
    pub(in crate::substitute) fn word_offset(self) -> usize {
        match self {
            Self::ShortestPrefix | Self::ShortestSuffix => 1,
            _ => 2,
        }
    }

    /// True for the prefix family (`#`, `##`), false for the suffix family.
    pub(in crate::substitute) fn prefix(self) -> bool {
        matches!(self, Self::ShortestPrefix | Self::LongestPrefix)
    }

    /// True for the longest-match operators (`##`, `%%`).
    pub(in crate::substitute) fn longest(self) -> bool {
        matches!(self, Self::LongestPrefix | Self::LongestSuffix)
    }
}
