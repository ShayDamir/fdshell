use sys::ShortCStr;

pub const NOCLOBBER: u32 = 1;
pub const EXPAND_ALIASES: u32 = 2;
pub const IGNOREEOF: u32 = 4;
pub const XTRACE: u32 = 8;
pub const BUILTIN_FIRST: u32 = 16;

/// All shell options, bash-compatible names.
///
/// The third field is the option's `set` short flag as shown by `$-`
/// (0 when the option has no short flag).
pub const OPTIONS: &[(&[u8], u32, u8)] = &[
    (b"noclobber", NOCLOBBER, b'C'),
    (b"expand_aliases", EXPAND_ALIASES, 0),
    (b"ignoreeof", IGNOREEOF, b'i'),
    (b"xtrace", XTRACE, b'x'),
    (b"builtin_first", BUILTIN_FIRST, 0),
];

/// The options on by default (bash: `expand_aliases` is on in interactive shells).
pub const DEFAULTS: u32 = EXPAND_ALIASES;

pub fn lookup(name: &ShortCStr) -> Option<u32> {
    OPTIONS
        .iter()
        .find(|(n, _, _)| name.eq_bytes(n))
        .map(|(_, bit, _)| *bit)
}

pub fn name_of(bit: u32) -> Option<&'static [u8]> {
    OPTIONS
        .iter()
        .find(|(_, b, _)| *b == bit)
        .map(|(n, _, _)| *n)
}

pub fn set(options: u32, bit: u32, on: bool) -> u32 {
    if on { options | bit } else { options & !bit }
}

/// The option list as `name on\n` / `name off\n` lines.
pub fn list(options: u32) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    for (name, bit, _) in OPTIONS {
        out.extend_from_slice(name);
        if options & *bit != 0 {
            out.extend_from_slice(b" on\n");
        } else {
            out.extend_from_slice(b" off\n");
        }
    }
    out
}

/// The active options' short flags in table order, for `$-` expansion.
pub fn flags(options: u32) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    for (_, bit, flag) in OPTIONS {
        if options & *bit != 0 && *flag != 0 {
            out.push(*flag);
        }
    }
    out
}

#[cfg(test)]
mod tests;
