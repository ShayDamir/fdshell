#![forbid(unsafe_code)]

use core::fmt;
use sys::fcntl::{O_APPEND, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY};

pub enum RedirectDirection {
    Read,
    Write,
    Append,
}

impl PartialEq for RedirectDirection {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl fmt::Debug for RedirectDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Append => "Append",
        })
    }
}

impl RedirectDirection {
    pub fn open_flags(&self) -> i32 {
        match self {
            Self::Read => O_RDONLY,
            Self::Write => O_WRONLY | O_CREAT | O_TRUNC,
            Self::Append => O_WRONLY | O_CREAT | O_APPEND,
        }
    }
}
