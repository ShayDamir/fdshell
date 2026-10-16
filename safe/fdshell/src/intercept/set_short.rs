use crate::error::cmd::CmdError;
use crate::state::ShellState;
use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

/// Split a `set` short-flag **cluster**: a `-`/`+` sign followed by one or more
/// known flag bytes, each applied with the word's sign (`set -eu` = errexit +
/// nounset, `set +ef` = clear noglob + verbosity). The bits come back in word
/// order, duplicates included (applying a flag twice is harmless).
///
/// `Ok(None)` is a word that is not ours: empty, signless (`set a b`), or a lone
/// sign (task #249's `set -`/`set +`) — the caller keeps falling through. A
/// signed cluster with an unknown byte is an error naming the whole word
/// (`set: unknown option '-z'`), and **no** flag has been applied yet: bash
/// validates the whole word first, so `set -ez` leaves `-e` off.
fn short_flags(first: &ShortCStr) -> Result<Option<(Vec<u32>, bool)>, Report<CmdError>> {
    let bytes = first.as_bytes().change_context(CmdError::Never)?;
    let Some(&sign) = bytes.first() else {
        return Ok(None);
    };
    let enable = sign == b'-';
    if !enable && sign != b'+' {
        return Ok(None);
    }
    // A lone sign has no flag byte, so it is not a cluster.
    let Some(flags) = bytes.get(1..).filter(|flags| !flags.is_empty()) else {
        return Ok(None);
    };

    let mut bits = Vec::new();
    for &flag in flags {
        bits.push(flag_bit(flag, first)?);
    }
    Ok(Some((bits, enable)))
}

/// The option bit of one cluster byte. The accepted bytes **are** the option
/// table's short flags — `e u f v x C i` ([`crate::options::flag_lookup`]) — so
/// a flag `set` accepts is a flag `$-` prints. `x` is accepted (bash's
/// `set -xf` works) even though the single `-x`/`+x` word is claimed by its own
/// `set` arm.
fn flag_bit(flag: u8, word: &ShortCStr) -> Result<u32, Report<CmdError>> {
    let bit = crate::options::flag_lookup(flag)
        .ok_or(CmdError::ShellOptionUnknown("set", word.clone()))?;
    Ok(bit)
}

/// `set -eu` / `set +ef`: validate the word, trace it, apply each flag byte with
/// the word's sign, status 0. Returns `false` for a word that is not a cluster
/// (the caller keeps falling through). The cluster is traced only if xtrace was
/// already on, as bash does (`set -xf` traces `echo`, not itself); the
/// unconditional self-trace belongs to the single `-x`/`+x` arm.
pub(super) fn run_short_flags(
    first: &ShortCStr,
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    let Some((bits, enable)) = short_flags(first)? else {
        return Ok(false);
    };
    super::validation::validate_intercept(line, "set", cmdline)?;
    crate::xtrace::trace_cmd(b"set", cmdline, cell);
    for bit in bits {
        run_set_short(bit, enable, cell)?;
    }
    Ok(true)
}

/// Apply one short-flag bit and report status 0 (the xtrace line is printed by
/// the caller, as for `set -o`).
fn run_set_short(
    bit: u32,
    enable: bool,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Never)?;
    state.options = crate::options::set(state.options, bit, enable);
    state.set_last_exit(0);
    Ok(())
}

#[cfg(test)]
mod tests;
