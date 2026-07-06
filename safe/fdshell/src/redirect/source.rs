use sys::ShortCStr;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectSource {
    Var(ShortCStr),
    Path(ShortCStr),
}

impl RedirectSource {
    pub fn var(name: impl Into<ShortCStr>) -> Self {
        Self::Var(name.into())
    }
    pub fn path(name: impl Into<ShortCStr>) -> Self {
        Self::Path(name.into())
    }
}
