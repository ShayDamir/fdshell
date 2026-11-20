// mutants: -C --lib -C --test -C redirect -- -E 'test(/^redirect::/) | binary(redirect)'

mod def;
mod direction;
mod heredoc;
mod herestring;
mod open;
mod opened;
mod resolve;
mod scope;
mod source;

pub use def::*;
pub use direction::*;
pub use open::*;
pub use opened::*;
pub use resolve::*;
pub use scope::*;
pub use source::*;

use error_stack::{Report, ResultExt};
use sys::{LocalFd, SyscallError};

use crate::error::redirect::OpenRedirectError;

/// The lowest fd number fdshell allocates for itself: bash keeps its own redirect
/// bookkeeping at fd >= 10, out of the range a script can name, so no `exec N>&-`
/// can close a saved copy or a source clone. The anti-clobber rule
/// (`max_target + 1`) still raises this floor for commands that name high fds.
pub(crate) const MIN_SHELL_FD: i32 = 10;

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

    /// The fd this redirection lands on.
    pub fn target(&self) -> i32 {
        match self {
            Self::Dup { export_to, .. } | Self::Close { export_to } => *export_to,
        }
    }

    pub fn export(&self) -> Result<(), Report<OpenRedirectError>> {
        match self {
            Self::Dup { local, export_to } => local
                .export_to(*export_to)
                .change_context(OpenRedirectError::Open)
                .map(|_| ()),
            Self::Close { export_to } => match sys::dup::close(*export_to) {
                // `EBADF` means the fd is not open, which is exactly the state the
                // def asked for — bash ignores that errno and continues, so it is a
                // silent success here too, not something the user can fix (§4.12).
                Ok(()) => Ok(()),
                Err(SyscallError::EBADF(_)) => Ok(()),
                // Only a live fd answers another errno here (`EIO`/`ENOSPC`/`EINTR`
                // on real devices); that close keeps its chain (§4.5).
                Err(e) => {
                    Err(Report::new(e).change_context(OpenRedirectError::CloseFd { n: *export_to }))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
