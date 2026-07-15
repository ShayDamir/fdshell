use alloc::vec::Vec;
use sys::ShortCStr;

pub(super) fn find_arg_pos(line: &[u8], args: &[ShortCStr], idx: usize) -> usize {
    args.get(idx)
        .and_then(|a| a.as_bytes().ok())
        .and_then(|bytes| line.windows(bytes.len()).position(|w| w == bytes))
        .unwrap_or(0)
}

pub(super) fn collect_values(args: &[ShortCStr], start: usize) -> (Vec<ShortCStr>, usize) {
    let mut values = Vec::new();
    let mut j = start;
    while let Some(next) = args.get(j) {
        if next.as_bytes().is_ok_and(|b| b.starts_with(b"--")) {
            break;
        }
        values.push(next.clone());
        j += 1;
    }
    (values, j)
}
