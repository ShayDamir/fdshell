use crate::child::{self, Command};
use crate::exec;
use crate::redirect::Redirect;
use crate::resolve::substitute_arg;
use crate::vars::FdVars;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use sys::ShortCStr;

pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    vars: &FdVars,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect],
) -> Result<(), i32> {
    if let Some(sock) = child_sock {
        sock.export_to(sys::shellfd::SHELLFD)?;
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }

    for r in redirects {
        let src = vars
            .resolve(r.src_var.as_bytes())
            .ok_or(sys::errno::EINVAL)?;
        src.export_to(r.target_fd)?;
    }

    let mut dup_cache: HashMap<ShortCStr, sys::ExportedFd> = HashMap::new();
    let resolved: Vec<CString> = args
        .iter()
        .map(|a| substitute_arg(a, &mut dup_cache, vars))
        .collect::<Result<_, _>>()?;

    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    match cmd {
        Command::Builtin(name) => child::builtin::dispatch_builtin(name, &refs, args, vars),
        Command::External(name) => {
            let name_cs = name.to_c_string();
            let fd = exec::resolve_path(&name_cs)?;
            let mut full_argv = Vec::with_capacity(refs.len() + 1);
            full_argv.push(name_cs.as_c_str());
            full_argv.extend(refs.iter().copied());
            exec::exec_fd(&fd, &full_argv)
        }
    }
}
