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
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn split_once_mid() {
        let (left, right) = split_once(b"hello=world", b"=").unwrap();
        assert_eq!(left, b"hello");
        assert_eq!(right, b"world");
    }

    #[test]
    fn split_once_start() {
        let (left, right) = split_once(b"=value", b"=").unwrap();
        assert_eq!(left, b"");
        assert_eq!(right, b"value");
    }

    #[test]
    fn split_once_end() {
        let (left, right) = split_once(b"prefix=", b"=").unwrap();
        assert_eq!(left, b"prefix");
        assert_eq!(right, b"");
    }

    #[test]
    fn split_once_none() {
        assert!(split_once(b"hello", b"=").is_none());
    }

    #[test]
    fn split_once_empty() {
        assert!(split_once(b"", b"=").is_none());
    }

    #[test]
    fn split_once_longer_than_data() {
        assert!(split_once(b"ab", b"abc").is_none());
    }

    #[test]
    fn split_once_multibyte_sep() {
        let (left, right) = split_once(b"Umask:\t0022", b"Umask:\t").unwrap();
        assert_eq!(left, b"");
        assert_eq!(right, b"0022");
    }

    #[test]
    fn split_once_repeated_sep() {
        let (left, right) = split_once(b"a=b=c", b"=").unwrap();
        assert_eq!(left, b"a");
        assert_eq!(right, b"b=c");
    }
}
