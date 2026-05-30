#![forbid(unsafe_code)]

use crate::child;
use crate::exec;
use crate::redirect::{Redirect, RedirectSource};
use crate::resolve::substitute_arg;
use crate::vars::FdVars;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use sys::ShortCStr;
use sys::fcntl::{O_CLOEXEC, O_CREAT};

pub fn replace_shell(
    args: &[ShortCStr],
    redirects: &[crate::redirect::RedirectDef],
    fdvars: &FdVars,
) -> ! {
    let result = execute(args, redirects, fdvars);
    match result {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}

fn execute(
    args: &[ShortCStr],
    redirects: &[crate::redirect::RedirectDef],
    fdvars: &FdVars,
) -> Result<(), i32> {
    let mut opened: Vec<sys::LocalFd> = Vec::with_capacity(redirects.len());
    for r in redirects {
        if let RedirectSource::Path(path) = &r.source {
            let flags = r.direction.open_flags();
            let name = path.to_c_string();
            let fd = sys::openat2::openat2(
                sys::atfd::AtFd::cwd(),
                &name,
                &sys::openat2::OpenHow::new(
                    (flags | O_CLOEXEC) as u64,
                    if flags & O_CREAT != 0 { 0o666 } else { 0 },
                ),
            )?;
            opened.push(fd);
        }
    }

    let mut resolved: Vec<Redirect<'_>> = Vec::with_capacity(redirects.len());
    let mut path_idx = 0usize;
    for r in redirects {
        let local = match &r.source {
            RedirectSource::Var(var) => fdvars.resolve(var.as_bytes()).ok_or(sys::errno::EINVAL)?,
            RedirectSource::Path(_) => {
                let fd = opened.get(path_idx).ok_or(sys::errno::EIO)?;
                path_idx += 1;
                fd
            }
        };
        resolved.push(r.resolve(local));
    }

    for r in &resolved {
        r.export()?;
    }

    sys::shellfd::set_capture_active(false);

    if args.first().map(|a| a.as_bytes()) == Some(b"builtin") {
        let builtin_name = args.get(1).ok_or(sys::errno::EINVAL)?;
        let builtin_args = args.get(2..).unwrap_or(&[]);
        let mut dup_cache: HashMap<ShortCStr, sys::ExportedFd> = HashMap::new();
        let substituted: Vec<CString> = builtin_args
            .iter()
            .map(|a| substitute_arg(a, &mut dup_cache, fdvars))
            .collect::<Result<_, _>>()?;
        let refs: Vec<&CStr> = substituted.iter().map(|cs| cs.as_c_str()).collect();
        child::builtin::dispatch_builtin(builtin_name.clone(), &refs, builtin_args, fdvars)
    } else {
        let binary = args.first().ok_or(sys::errno::EINVAL)?;
        let binary_cs = binary.to_c_string();
        let fd = exec::resolve_path(&binary_cs)?;
        let mut dup_cache: HashMap<ShortCStr, sys::ExportedFd> = HashMap::new();
        let substituted: Vec<CString> = args
            .get(1..)
            .unwrap_or(&[])
            .iter()
            .map(|a| substitute_arg(a, &mut dup_cache, fdvars))
            .collect::<Result<_, _>>()?;
        let mut argv = Vec::with_capacity(substituted.len() + 1);
        argv.push(binary_cs.as_c_str());
        for s in &substituted {
            argv.push(s.as_c_str());
        }
        exec::exec_fd(&fd, &argv)
    }
}
