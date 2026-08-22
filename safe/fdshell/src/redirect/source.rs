use sys::ShortCStr;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectSource {
    Var(ShortCStr),
    Path(ShortCStr),
    HereString(ShortCStr),
    /// Dup from an already-open fd number (`2>&1`).
    Dup(i32),
    /// Close the target fd (`2>&-`).
    Close,
}

impl RedirectSource {
    pub fn var(name: impl Into<ShortCStr>) -> Self {
        Self::Var(name.into())
    }
    pub fn path(name: impl Into<ShortCStr>) -> Self {
        Self::Path(name.into())
    }
    pub fn here_string(word: impl Into<ShortCStr>) -> Self {
        Self::HereString(word.into())
    }
    pub fn dup(from: i32) -> Self {
        Self::Dup(from)
    }
    pub fn close() -> Self {
        Self::Close
    }
}
