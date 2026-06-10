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
    let bytes = arg.as_bytes()?;
    let mut out = Vec::new();
    let mut peek = bytes.iter().copied().peekable();
    if bytes.first() == Some(&b'~') {
        peek.next();
        match peek.peek() {
            None | Some(&b'/') => {
                if let Ok(home) = std::env::var("HOME") {
                    out.extend_from_slice(home.as_bytes());
                }
            }
            _ => out.push(b'~'),
        }
    }
    while let Some(b) = peek.next() {
        match b {
            b'%' => percent::percent_subst(&mut peek, cache, state, &mut out)?,
            b'$' if peek.peek() == Some(&b'(') => {
                peek.next();
                let inner = read_paren_expr(&mut peek)?;
                let expanded = crate::cmd_subst::run_and_capture(&inner, state)?;
                out.extend_from_slice(&expanded);
            }
            b'$' => dollar::dollar_subst(&mut peek, state, &mut out)?,
            _ => out.push(b),
        }
    }
    CString::new(out).map_err(|_| sys::errno::EINVAL)
}

fn read_paren_expr(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
) -> Result<Vec<u8>, i32> {
    let mut inner = Vec::new();
    let mut depth = 1u32;
    while depth > 0 {
        match peek.peek().copied() {
            Some(b'(') => {
                inner.push(b'(');
                depth += 1;
                peek.next();
            }
            Some(b')') => {
                depth -= 1;
                if depth == 0 {
                    peek.next();
                    break;
                }
                inner.push(b')');
                peek.next();
            }
            Some(c) => {
                inner.push(c);
                peek.next();
            }
            None => return Err(sys::errno::EINVAL),
        }
    }
    Ok(inner)
}

pub fn substitute_args(args: &[ShortCStr], state: &ShellState) -> Result<Vec<CString>, i32> {
    let mut cache: HashMap<ShortCStr, ExportedFd> = HashMap::new();
    args.iter()
        .map(|a| substitute_arg(a, &mut cache, state))
        .collect()
}

#[cfg(test)]
mod tests;
