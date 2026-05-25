#![forbid(unsafe_code)]

use crate::vars::Vars;
use std::ffi::CString;
use sys::errno::EEXIST;

pub struct Capture {
    pub var: CString,
    pub tag: Option<CString>,
    pub force: bool,
}

pub fn do_captures(
    capture_fd: sys::Fd,
    captures: &mut Vec<Capture>,
    vars: &mut Vars,
) -> Result<(), i32> {
    while !captures.is_empty() {
        let mut buf = [0u8; sys::shellfd::TAG_MAX];
        let (fd, rtag) = sys::shellfd::recv_fd(&capture_fd, &mut buf)?;

        let idx = captures
            .iter()
            .position(|c| {
                c.tag
                    .as_ref()
                    .is_some_and(|t| t.as_bytes() == rtag.to_bytes())
            })
            .or_else(|| captures.iter().position(|c| c.tag.is_none()));

        match idx {
            Some(i) => {
                let c = captures.remove(i);
                if !c.force && vars.resolve(c.var.as_c_str()).is_some() {
                    fd.close()?;
                    return Err(EEXIST);
                }
                vars.insert(c.var, fd);
            }
            None => {
                fd.close()?;
            }
        }
    }
    Ok(())
}
