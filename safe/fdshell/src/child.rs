#![forbid(unsafe_code)]
use crate::redirect::Redirect;
use crate::resolve::substitute_arg;
use crate::vars::FdVars;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use sys::ShortCStr;
pub enum Command {
    Builtin(ShortCStr),
    External(ShortCStr),
}
pub fn child_exec(
    child_sock: sys::Fd,
    vars: &FdVars,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect],
) -> ! {
    match child_main(child_sock, vars, cmd, args, redirects) {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}

fn child_main(
    child_sock: sys::Fd,
    vars: &FdVars,
    cmd: Command,
    args: &[ShortCStr],
    redirects: &[Redirect],
) -> Result<(), i32> {
    child_sock.dup_to(sys::shellfd::SHELLFD)?;

    for r in redirects {
        let src = vars
            .resolve(r.src_var.as_bytes())
            .ok_or(sys::errno::EINVAL)?;
        src.dup_to(r.target_fd)?;
    }

    let mut dup_cache: HashMap<ShortCStr, sys::DupFd> = HashMap::new();
    let resolved: Vec<CString> = args
        .iter()
        .map(|a| substitute_arg(a, &mut dup_cache, vars))
        .collect::<Result<_, _>>()?;

    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    match cmd {
        Command::Builtin(name) => match name.as_bytes() {
            b"echo" => {
                for (i, arg) in refs.iter().enumerate() {
                    if i > 0 {
                        print!(" ");
                    }
                    print!("{}", arg.to_str().map_err(|_| sys::errno::EINVAL)?);
                }
                println!();
                Ok(())
            }
            b"pipe" => builtins::pipe::parse::pipe_parse(&refs)
                .and_then(|cfg| builtins::pipe::pipe_exec(cfg.flags)),
            b"mkdirat" => builtins::mkdirat::parse::mkdirat_parse(&refs)
                .and_then(|cfg| builtins::mkdirat::mkdirat_exec(&cfg)),
            b"openat2" => builtins::openat2::parse::openat2_parse(&refs)
                .and_then(|cfg| builtins::openat2::openat2_exec(&cfg)),
            b"renameat2" => builtins::renameat2::parse::renameat2_parse(&refs)
                .and_then(|cfg| builtins::renameat2::renameat2_exec(&cfg)),
            _ => Err(sys::errno::ENOSYS),
        },
        Command::External(_) => todo!(),
    }
}
