// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::umask_mode::/) | binary(umask)'

//! The two byte classes of one clause (task #554): the `who` letters that open it and
//! the letters of the action run after its operator. Both answer `None` for a byte the
//! class does not own, which is what ends the run — the walker in the parent keeps no
//! table of its own.

use crate::parse::umask_mode::symbolic::Step;

/// The class letters of the `who` part; any other byte ends that part, so the
/// operator is what comes next (and the default `who` is `a`).
pub(crate) fn who_bit(byte: u8) -> Option<u32> {
    match byte {
        b'u' => Some(0o700),
        b'g' => Some(0o070),
        b'o' => Some(0o007),
        b'a' => Some(0o777),
        _ => None,
    }
}

/// One letter of an action run. `s` and `t` carry only bits above `0777`, which
/// `perm &= who` erases, so they are the empty step: accepted and doing nothing.
pub(crate) fn perm_step(byte: u8) -> Option<Step> {
    match byte {
        b'u' => Some(Step::Copy(0o700)),
        b'g' => Some(Step::Copy(0o070)),
        b'o' => Some(Step::Copy(0o007)),
        b'r' => Some(Step::Bits(0o444)),
        b'w' => Some(Step::Bits(0o222)),
        b'X' => Some(Step::X),
        b'x' => Some(Step::Bits(0o111)),
        b's' | b't' => Some(Step::Bits(0)),
        _ => None,
    }
}
