use sys::ShortCStr;

pub fn is_builtin(t: &ShortCStr) -> bool {
    t.as_bytes()
        .is_ok_and(|b| matches!(b, b"true" | b"false" | b"pwd" | b"help"))
}
