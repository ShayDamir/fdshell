#![forbid(unsafe_code)]

use core::fmt;
use sys::LocalFd;
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

pub enum RedirectDirection {
    Read,
    Write,
}

impl PartialEq for RedirectDirection {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl fmt::Debug for RedirectDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "Read"),
            Self::Write => write!(f, "Write"),
        }
    }
}

pub struct RedirectDef {
    pub export_to: i32,
    pub direction: RedirectDirection,
    pub source: RedirectSource,
}

impl PartialEq for RedirectDef {
    fn eq(&self, other: &Self) -> bool {
        self.export_to == other.export_to
            && self.direction == other.direction
            && self.source == other.source
    }
}

impl fmt::Debug for RedirectDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedirectDef")
            .field("export_to", &self.export_to)
            .field("direction", &self.direction)
            .field("source", &self.source)
            .finish()
    }
}

pub struct Redirect<'a> {
    pub export_to: i32,
    pub local: &'a LocalFd,
}

impl Redirect<'_> {
    pub fn export(&self) -> Result<(), i32> {
        self.local.export_to(self.export_to)?;
        Ok(())
    }
}
