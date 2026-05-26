#![forbid(unsafe_code)]

use crate::vars::FdVars;
use std::collections::HashMap;
use std::ffi::CString;
use sys::DupFd;
use sys::ShortCStr;

pub(crate) fn substitute_arg(
    arg: &ShortCStr,
    cache: &mut HashMap<ShortCStr, DupFd>,
    vars: &FdVars,
) -> Result<CString, i32> {
    let mut out = Vec::new();
    let mut peek = arg.as_bytes().iter().copied().peekable();
    while let Some(b) = peek.next() {
        if b != b'%' {
            out.push(b);
            continue;
        }
        match peek.peek().copied() {
            Some(b'%') => {
                out.push(b'%');
                peek.next();
            }
            Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
                let mut name = Vec::new();
                name.push(c);
                peek.next();
                while let Some(&nc) = peek.peek() {
                    if nc.is_ascii_alphanumeric() || nc == b'_' {
                        name.push(nc);
                        peek.next();
                    } else {
                        break;
                    }
                }
                let name_scs = ShortCStr::from_vec(name)?;
                let raw = match cache.get(&name_scs) {
                    Some(d) => d.as_raw(),
                    None => {
                        let src = vars
                            .resolve(name_scs.as_c_str())
                            .ok_or(sys::errno::EINVAL)?;
                        let d = src.dup()?;
                        let raw = d.as_raw();
                        cache.insert(name_scs, d);
                        raw
                    }
                };
                let num_str = format!("{}", raw);
                out.extend_from_slice(num_str.as_bytes());
            }
            _ => out.push(b'%'),
        }
    }
    CString::new(out).map_err(|_| sys::errno::EINVAL)
}
