use sys::ShortCStr;

pub const NOCLOBBER: u32 = 1;
pub const EXPAND_ALIASES: u32 = 2;

/// All shell options, bash-compatible names.
pub const OPTIONS: &[(&[u8], u32)] = &[
    (b"noclobber", NOCLOBBER),
    (b"expand_aliases", EXPAND_ALIASES),
];

/// The options on by default (bash: `expand_aliases` is on in interactive shells).
pub const DEFAULTS: u32 = EXPAND_ALIASES;

pub fn lookup(name: &ShortCStr) -> Option<u32> {
    OPTIONS
        .iter()
        .find(|(n, _)| name.eq_bytes(n))
        .map(|(_, bit)| *bit)
}

pub fn name_of(bit: u32) -> Option<&'static [u8]> {
    OPTIONS.iter().find(|(_, b)| *b == bit).map(|(n, _)| *n)
}

pub fn set(options: u32, bit: u32, on: bool) -> u32 {
    if on { options | bit } else { options & !bit }
}

/// The option list as `name on\n` / `name off\n` lines.
pub fn list(options: u32) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    for (name, bit) in OPTIONS {
        out.extend_from_slice(name);
        if options & *bit != 0 {
            out.extend_from_slice(b" on\n");
        } else {
            out.extend_from_slice(b" off\n");
        }
    }
    out
}

#[cfg(test)]
mod tests;
