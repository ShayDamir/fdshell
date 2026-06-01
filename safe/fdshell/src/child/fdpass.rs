use crate::vars::FdVars;
use std::ffi::CString;
use sys::ShortCStr;

pub fn dispatch(name: &[u8], args: &[ShortCStr], vars: &FdVars) -> Option<Result<(), i32>> {
    match name {
        b"import_fd" => Some(import_fd(args)),
        b"export_fd" => Some(export_fd(args, vars)),
        _ => None,
    }
}

fn import_fd(args: &[ShortCStr]) -> Result<(), i32> {
    let raw = args.first().ok_or(sys::errno::EINVAL)?;
    let fd = sys::ImportedFd::from_bytes(raw.as_bytes())?;
    sys::shellfd::send_fd(&fd.try_into_local()?, c"import_fd")
}

pub(crate) fn export_fd(args: &[ShortCStr], vars: &FdVars) -> Result<(), i32> {
    let (vname, tag) = match args {
        [a] => {
            let v = a.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let tag = CString::new(v.as_bytes().to_vec()).map_err(|_| sys::errno::EINVAL)?;
            (v, tag)
        }
        [t, v] => {
            if t.as_bytes().contains(&b'%') {
                return Err(sys::errno::EINVAL);
            }
            let tag = t.to_c_string();
            let v = v.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            (v, tag)
        }
        _ => return Err(sys::errno::EINVAL),
    };
    let fd = vars.resolve(vname.as_bytes()).ok_or(sys::errno::EINVAL)?;
    sys::shellfd::send_fd(fd, &tag)
}
