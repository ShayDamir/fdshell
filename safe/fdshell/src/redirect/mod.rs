mod def;
mod direction;
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

pub struct Redirect {
    pub export_to: i32,
    pub local: LocalFd,
}

impl Redirect {
    pub fn new(export_to: i32, local: LocalFd) -> Self {
        Redirect { export_to, local }
    }

    pub fn export(&self) -> Result<(), Report<OpenRedirectError>> {
        self.local
            .export_to(self.export_to)
            .change_context(OpenRedirectError)?;
        Ok(())
    }
}
