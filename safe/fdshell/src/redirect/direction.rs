use sys::fcntl::{O_APPEND, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY};

#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectDirection {
    Read,
    Write,
    Append,
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
