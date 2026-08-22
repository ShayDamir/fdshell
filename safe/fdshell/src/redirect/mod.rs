mod def;
mod direction;
mod herestring;
mod open;
mod resolve;
mod source;

pub use def::*;
pub use direction::*;
pub use open::*;
pub use resolve::*;
pub use source::*;

use error_stack::{Report, ResultExt};
use sys::LocalFd;

use crate::error::redirect::OpenRedirectError;

/// A resolved redirection: either a local fd to dup2 onto `export_to`,
/// or a request to close `export_to` (`N>&-`).
pub enum Redirect {
    Dup { export_to: i32, local: LocalFd },
    Close { export_to: i32 },
}

impl Redirect {
    pub fn new(export_to: i32, local: LocalFd) -> Self {
        Redirect::Dup { export_to, local }
    }

    pub fn export(&self) -> Result<(), Report<OpenRedirectError>> {
        match self {
            Self::Dup { local, export_to } => local
                .export_to(*export_to)
                .change_context(OpenRedirectError::Open)
                .map(|_| ()),
            Self::Close { export_to } => sys::close::close(*export_to)
                .change_context_lazy(|| OpenRedirectError::CloseFd { n: *export_to }),
        }
    }
}
