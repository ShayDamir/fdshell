use core::convert::TryFrom;
use core::ffi::CStr;

use error_stack::{Report, ResultExt};

use crate::shortcstr::ShortCStr;

impl TryFrom<&CStr> for crate::ImportedFd {
    type Error = Report<crate::ImportedFdError>;
    fn try_from(s: &CStr) -> Result<Self, Self::Error> {
        Self::from_bytes(s.to_bytes())
    }
}

impl TryFrom<&ShortCStr> for crate::ImportedFd {
    type Error = Report<crate::ImportedFdError>;
    fn try_from(scs: &ShortCStr) -> Result<Self, Self::Error> {
        let bytes = scs
            .as_bytes()
            .change_context(crate::ImportedFdError::NotANumber)?;
        Self::from_bytes(bytes)
    }
}
