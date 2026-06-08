use crate::exec;
use crate::state::ShellState;
use std::ffi::CStr;
use sys::ShortCStr;

pub fn dispatch_builtin(
    name: ShortCStr,
    refs: &[&CStr],
    args: &[ShortCStr],
    state: &ShellState,
) -> Result<(), i32> {
    match name.as_bytes()? {
        b"fchmod" => builtins::fchmod::parse::fchmod_parse(refs)
            .and_then(|cfg| builtins::fchmod::fchmod_exec(&cfg)),
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
        b"pipe" => builtins::pipe::parse::pipe_parse(refs)
            .and_then(|cfg| builtins::pipe::pipe_exec(cfg.flags)),
        b"mkdirat" => builtins::mkdirat::parse::mkdirat_parse(refs)
            .and_then(|cfg| builtins::mkdirat::mkdirat_exec(&cfg)),
        b"openat2" => builtins::openat2::parse::openat2_parse(refs)
            .and_then(|cfg| builtins::openat2::openat2_exec(&cfg)),
        b"renameat2" => builtins::renameat2::parse::renameat2_parse(refs)
            .and_then(|cfg| builtins::renameat2::renameat2_exec(&cfg)),
        b"exec_fd" => {
            let raw0 = args.first().ok_or(sys::errno::EINVAL)?;
            let varname = raw0.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let fd = state.fds.get(&varname).ok_or(sys::errno::EINVAL)?;
            exec::exec_fd(fd, refs.get(1..).ok_or(sys::errno::EINVAL)?)
        }
        b"exec_at" => {
            let raw0 = args.first().ok_or(sys::errno::EINVAL)?;
            let varname = raw0.strip_prefix(b"%").ok_or(sys::errno::EINVAL)?;
            let dirfd = state.fds.get(&varname).ok_or(sys::errno::EINVAL)?;
            let pathname = args.get(1).ok_or(sys::errno::EINVAL)?;
            let pathname = sys::RefCStr::from(pathname.clone());
            // execveat with a relative pathname rejects dirfds that have FD_CLOEXEC set.
            // Create a non-CLOEXEC copy via export().
            let non_cloexec = dirfd.export()?;
            exec::exec_at(
                non_cloexec.at(),
                &pathname,
                refs.get(2..).ok_or(sys::errno::EINVAL)?,
            )
        }
        b"resolve" => {
            let name = refs.first().ok_or(sys::errno::EINVAL)?;
            let fd = exec::resolve_path(name)?;
            sys::shellfd::send_fd(&fd, c"resolve").ok();
            Ok(())
        }
        name_bytes => crate::child::fdpass::dispatch(name_bytes, args, state)
            .unwrap_or(Err(sys::errno::ENOSYS)),
    }
}
