use crate::vars::FdVars;
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
    let fd = sys::ImportedFd::try_from(raw)?;
    sys::shellfd::send_fd(&fd.try_into_local()?, c"import_fd")
}

pub(crate) fn export_fd(args: &[ShortCStr], vars: &FdVars) -> Result<(), i32> {
    let (vname, tag) = match args {
        [a] => {
            let v = a.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let tag = sys::RefCStr::from(v.clone());
            (v, tag)
        }
        [t, v] => {
            if t.contains(b'%') {
                return Err(sys::errno::EINVAL);
            }
            let tag = sys::RefCStr::from(t.clone());
            let v = v.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            (v, tag)
        }
        _ => return Err(sys::errno::EINVAL),
    };
    let fd = vars.resolve(&vname).ok_or(sys::errno::EINVAL)?;
    sys::shellfd::send_fd(fd, &tag)
}
