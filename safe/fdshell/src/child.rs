#![forbid(unsafe_code)]
use crate::vars::Vars;
use std::ffi::{CStr, CString};
pub enum Command {
    Builtin(CString),
    External(CString),
}
pub fn child_exec(child_sock: sys::Fd, vars: &Vars, cmd: Command, args: &[CString]) -> ! {
    match child_main(child_sock, vars, cmd, args) {
        Ok(()) => std::process::exit(0),
        Err(e) => std::process::exit(e),
    }
}

fn child_main(child_sock: sys::Fd, vars: &Vars, cmd: Command, args: &[CString]) -> Result<(), i32> {
    child_sock.dup2(sys::shellfd::SHELL_DUPFD)?;

    let resolved: Vec<CString> = args
        .iter()
        .map(|a| resolve_one_arg(a, vars))
        .collect::<Result<_, _>>()?;

    let refs: Vec<&CStr> = resolved.iter().map(|cs| cs.as_c_str()).collect();

    match cmd {
        Command::Builtin(name) => match name.as_bytes() {
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

fn resolve_one_arg(arg: &CStr, vars: &Vars) -> Result<CString, i32> {
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
