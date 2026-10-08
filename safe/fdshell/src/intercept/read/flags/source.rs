use sys::{Origin, ShortCStr};

#[derive(Debug)]
pub(crate) enum SourceFd {
    Stdin,
    RawFd(ShortCStr),
    FdVar(ShortCStr),
}

impl SourceFd {
    /// The origin tag for values read from this source.
    pub(crate) fn origin(&self) -> Origin {
        match self {
            SourceFd::FdVar(var) => Origin::Read(var.clone()),
            SourceFd::RawFd(num) => Origin::Read(num.clone()),
            SourceFd::Stdin => Origin::Stdin,
        }
    }
}
