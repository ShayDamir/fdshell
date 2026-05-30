use sys::ShortCStr;

use super::{RedirectDirection, RedirectSource};

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct RedirectDef {
    pub export_to: i32,
    pub direction: RedirectDirection,
    pub source: RedirectSource,
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
