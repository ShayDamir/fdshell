use sys::ShortCStr;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectSource {
    Var(ShortCStr),
    Path(ShortCStr),
    HereString(ShortCStr),
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
}
