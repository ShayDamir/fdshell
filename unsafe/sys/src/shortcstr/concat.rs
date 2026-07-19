use crate::shortcstr::{ShortCStr, ShortCStrError};

impl ShortCStr {
    /// Concatenate all input slices into a new `ShortCStr`.
    ///
    /// Returns an error if any input contains a NUL byte.
    pub fn concat(parts: &[&ShortCStr]) -> Result<ShortCStr, ShortCStrError> {
        parts.iter().try_fold(ShortCStr::new(), |mut acc, part| {
            acc.push_str(part)?;
            Ok(acc)
        })
    }
}
