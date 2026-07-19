use alloc::vec::Vec;
use sys::ShortCStr;

pub(super) fn find_arg_pos(line: &[u8], args: &[ShortCStr], idx: usize) -> usize {
    args.get(idx)
        .and_then(|a| a.as_bytes().ok())
        .and_then(|bytes| line.windows(bytes.len()).position(|w| w == bytes))
        .unwrap_or(0)
}

pub(super) fn collect_values(
    args: &[ShortCStr],
    start: usize,
    patterns: &mut Vec<ShortCStr>,
) -> usize {
    let before = patterns.len();
    patterns.extend(
        args.iter()
            .skip(start)
            .take_while(|v| !v.starts_with(b"--"))
            .cloned(),
    );
    start + patterns.len() - before
}
