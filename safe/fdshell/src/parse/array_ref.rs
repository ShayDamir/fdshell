use sys::ShortCStr;

/// Split `name[idx]` into `(name, idx)` when the tail is exactly `[digits]`.
pub(crate) fn split_index_ref(name: &ShortCStr) -> Option<(ShortCStr, usize)> {
    let open = name.find_byte(b'[')?;
    let rest = name.get(open + 1..)?;
    let close = rest.find_byte(b']')?;
    let digits = rest.get(0..close)?;
    let idx = digits.parse::<usize>().ok()?;
    if rest.get(close + 1..).is_some_and(|r| !r.is_empty()) {
        return None;
    }
    let base = name.get(0..open)?;
    if base.is_empty() {
        return None;
    }
    Some((base, idx))
}

/// Split `arr[%name]` into `(arr, name)` when the tail is exactly `[%name]`.
pub(crate) fn split_element_ref(var: &ShortCStr) -> Option<(ShortCStr, ShortCStr)> {
    let open = var.find_byte(b'[')?;
    let rest = var.get(open + 1..)?;
    let close = rest.find_byte(b']')?;
    if rest.get(close + 1..).is_some_and(|r| !r.is_empty()) {
        return None;
    }
    let inner = rest.get(0..close)?.strip_prefix(b"%")?;
    if inner.is_empty() {
        return None;
    }
    let base = var.get(0..open)?;
    if base.is_empty() {
        return None;
    }
    Some((base, inner))
}
