#![forbid(unsafe_code)]

use core::fmt;
use sys::ShortCStr;

pub enum RedirectSource {
    Var(ShortCStr),
    Path(ShortCStr),
}

impl PartialEq for RedirectSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Var(a), Self::Var(b)) => a == b,
            (Self::Path(a), Self::Path(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Debug for RedirectSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Var(v) => f.debug_tuple("Var").field(v).finish(),
            Self::Path(p) => f.debug_tuple("Path").field(p).finish(),
        }
    }
}

impl RedirectSource {
    pub fn var(name: impl Into<ShortCStr>) -> Self {
        Self::Var(name.into())
    }
    pub fn path(name: impl Into<ShortCStr>) -> Self {
        Self::Path(name.into())
    }
}
