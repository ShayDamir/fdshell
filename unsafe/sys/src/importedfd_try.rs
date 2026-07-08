use core::convert::TryFrom;
use core::ffi::CStr;

use crate::shortcstr::ShortCStr;
use error_stack::ResultExt;

// Single-source conversion: no Report needed, used by `builtins` (no_std, i32 errors).
impl TryFrom<&CStr> for crate::ImportedFd {
    type Error = crate::SyscallError;
    fn try_from(s: &CStr) -> Result<Self, crate::SyscallError> {
        Self::from_bytes(s.to_bytes())
    }
}

// Two error sources (as_bytes + from_bytes): Report chains both for fdshell callers.
impl TryFrom<&ShortCStr> for crate::ImportedFd {
    type Error = error_stack::Report<crate::SyscallError>;
    fn try_from(scs: &ShortCStr) -> Result<Self, error_stack::Report<crate::SyscallError>> {
        let bytes = scs.as_bytes().change_context(crate::SyscallError::EINVAL)?;
        Self::from_bytes(bytes).change_context(crate::SyscallError::EINVAL)
    }
}
