use core::str::FromStr;

use error_stack::{Report, ResultExt};

use crate::shortcstr::{ShortCStr, ShortCStrError};

impl ShortCStr {
    /// Parse the contents as a value of type `T`.
    ///
    /// The bytes are first validated as UTF-8, then passed to
    /// [`FromStr::from_str`]. Returns [`ShortCStrError::InvalidUtf8`] if
    /// the bytes are not valid UTF-8, or [`ShortCStrError::FromStrFailed`]
    /// if they cannot be parsed as `T`.
    pub fn parse<T: FromStr>(&self) -> Result<T, Report<ShortCStrError>>
    where
        T::Err: core::error::Error + Send + Sync + 'static,
    {
        let bytes = self.as_bytes()?;
        let s = core::str::from_utf8(bytes).change_context(ShortCStrError::InvalidUtf8)?;
        T::from_str(s).change_context(ShortCStrError::FromStrFailed)
    }
}
