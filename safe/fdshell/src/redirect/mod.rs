#![forbid(unsafe_code)]

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

use sys::LocalFd;

pub struct Redirect<'a> {
    pub export_to: i32,
    pub local: &'a LocalFd,
}

impl<'a> Redirect<'a> {
    pub fn new(export_to: i32, local: &'a LocalFd) -> Self {
        Redirect { export_to, local }
    }

    pub fn export(&self) -> Result<(), i32> {
        self.local.export_to(self.export_to)?;
        Ok(())
    }
}
