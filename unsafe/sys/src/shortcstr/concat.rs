use crate::shortcstr::ShortCStr;

impl ShortCStr {
    /// Concatenate all input slices into a new `ShortCStr`.
    pub fn concat(parts: &[&ShortCStr]) -> ShortCStr {
        parts.iter().fold(ShortCStr::new(), |mut acc, part| {
            acc.push_str(part);
            acc
        })
    }
}
