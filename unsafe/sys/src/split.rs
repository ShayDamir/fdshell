/// Split a byte slice at the first occurrence of `sep`.
///
/// Returns `None` if `sep` is not found or is longer than `data`.
pub(crate) fn split_once<'a>(data: &'a [u8], sep: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
    data.windows(sep.len())
        .position(|w| w == sep)
        .and_then(|i| {
            let left = data.get(..i)?;
            let right = data.get(i + sep.len()..)?;
            Some((left, right))
        })
}

#[cfg(test)]
mod tests;
