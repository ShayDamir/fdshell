use crate::child::{self, Command};
use crate::exec;
use crate::redirect::Redirect;
use crate::resolve::substitute_args;
use crate::vars::FdVars;
use std::ffi::CStr;
use sys::ShortCStr;

pub fn child_main(
    child_sock: Option<sys::LocalFd>,
    vars: &FdVars,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect<'_>],
) -> Result<(), i32> {
    if let Some(sock) = child_sock {
        sock.export_to(sys::shellfd::SHELLFD)?;
        sys::shellfd::set_capture_active(true);
    } else {
        sys::shellfd::set_capture_active(false);
    }

    for r in redirects {
        r.export()?;
    }

    let resolved = substitute_args(args, vars)?;
    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    match cmd {
        Command::Builtin(name) => child::builtin::dispatch_builtin(name, &refs, args, vars),
        Command::External(name) => {
            let name = sys::RefCStr::from(name.clone());
            let fd = exec::resolve_path(&name)?;
            let full_argv: Vec<&CStr> = std::iter::once(name.as_ref())
                .chain(refs.iter().copied())
                .collect();
            exec::exec_fd(&fd, &full_argv)
        }
    }
}
