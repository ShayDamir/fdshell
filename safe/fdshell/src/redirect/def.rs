use core::fmt;
use sys::ShortCStr;

use super::{RedirectDirection, RedirectSource};

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

impl RedirectDef {
    pub fn var(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Write,
            source: RedirectSource::var(name),
        }
    }

    pub fn read_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Read,
            source: RedirectSource::path(name),
        }
    }

    pub fn write_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Write,
            source: RedirectSource::path(name),
        }
    }

    pub fn append_path(export_to: i32, name: impl Into<ShortCStr>) -> Self {
        RedirectDef {
            export_to,
            direction: RedirectDirection::Append,
            source: RedirectSource::path(name),
        }
    }

    pub fn resolve<'a>(&self, local: &'a super::LocalFd) -> super::Redirect<'a> {
        super::Redirect {
            export_to: self.export_to,
            local,
        }
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
