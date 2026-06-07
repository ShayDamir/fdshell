#![forbid(unsafe_code)]

mod dollar;
mod percent;

use std::collections::HashMap;
use std::ffi::CString;
use sys::ExportedFd;
use sys::ShortCStr;

use crate::state::ShellState;

pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    cache: &mut HashMap<ShortCStr, ExportedFd>,
    state: &ShellState,
) -> Result<CString, i32> {
    let mut out = Vec::new();
    let mut peek = arg.as_bytes()?.iter().copied().peekable();
    while let Some(b) = peek.next() {
        match b {
            b'%' => percent::percent_subst(&mut peek, cache, state, &mut out)?,
            b'$' => dollar::dollar_subst(&mut peek, state, &mut out)?,
            _ => out.push(b),
        }
    }
    CString::new(out).map_err(|_| sys::errno::EINVAL)
}

pub fn substitute_args(args: &[ShortCStr], state: &ShellState) -> Result<Vec<CString>, i32> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    args.iter()
        .map(|a| substitute_arg(a, &mut cache, state))
        .collect()
}

#[cfg(test)]
mod tests;
