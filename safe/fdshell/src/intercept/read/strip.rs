use sys::ShortCStr;

pub(crate) fn strip_prefix(name: &ShortCStr) -> ShortCStr {
    let bytes = name.as_bytes().unwrap_or(&[]);
    if let Some(rest) = bytes.strip_prefix(b"$") {
        ShortCStr::from_vec(rest.to_vec()).unwrap_or_else(|_| ShortCStr::new())
    } else {
        name.clone()
    }
}
