use sys::fcntl::{O_APPEND, O_CREAT, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY};

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum RedirectDirection {
    Read,
    Write,
    Append,
    Rw,
    /// `>|` — a write that bypasses `noclobber` (POSIX #2.2). Same flags as
    /// [`Self::Write`]: the bypass is the `open.rs` gate, which keys on
    /// `Write` alone, so a direction that is not `Write` opens freely.
    Clobber,
}

impl RedirectDirection {
    // The O_* flags are disjoint single bits, so summing is identical to OR.
    // `|` here produces equivalent mutants under mutation testing.
    pub fn open_flags(&self) -> i32 {
        match self {
            Self::Read => O_RDONLY,
            Self::Write | Self::Clobber => O_WRONLY + O_CREAT + O_TRUNC,
            Self::Append => O_WRONLY + O_CREAT + O_APPEND,
            Self::Rw => O_RDWR + O_CREAT,
        }
    }
}

#[cfg(test)]
mod tests;
