#![forbid(unsafe_code)]

use crate::vars::Vars;
use std::ffi::{CStr, CString};

pub(crate) fn substitute_arg(arg: &CStr, vars: &Vars) -> Result<CString, i32> {
    let mut out = Vec::new();
    let mut peek = arg.to_bytes().iter().copied().peekable();
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
                if let Some(fd) = vars.resolve(&CString::new(name).map_err(|_| sys::errno::EINVAL)?)
                {
                    let num_str = format!("{}", fd.dup()?.as_raw());
                    out.extend_from_slice(num_str.as_bytes());
                } else {
                    return Err(sys::errno::EINVAL);
                }
            }
            _ => out.push(b'%'),
        }
    }
    CString::new(out).map_err(|_| sys::errno::EINVAL)
}
