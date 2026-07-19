use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::{ExportedCStr, ExportedFd, ShortCStr};

use crate::envfilter::EnvFilter;

pub(crate) fn get_environ(
    pid: i32,
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    exec_sock: Option<&ExportedFd>,
) -> Vec<ExportedCStr> {
    let env_iter = sys::env::environ_snapshot()
        .into_iter()
        .filter_map(|(k, v)| {
            let key = k.as_bytes().ok()?;
            key.ne(b"FDSHELL_PID").then_some(())?;
            key.ne(b"FDSHELL_SOCKET").then_some(())?;
            env_filter.is_allowed(key).then_some(())?;
            Some(ExportedCStr::from(
                ShortCStr::concat(&[&k, &c"=".into(), &v]).ok()?,
            ))
        });
    let exports_iter = exports.iter().filter_map(|(k, v)| {
        if let Ok(key) = k.as_bytes() {
            if !env_filter.is_allowed(key) {
                return None;
            }
            Some(ExportedCStr::from(
                ShortCStr::concat(&[k, &c"=".into(), v]).ok()?,
            ))
        } else {
            None
        }
    });

    let pid_entry = sys::format!("{pid}").ok().and_then(|pid_str| {
        ShortCStr::concat(&[&c"FDSHELL_PID=".into(), &pid_str])
            .map(ExportedCStr::from)
            .ok()
    });

    let sock_entry = exec_sock.and_then(|s| {
        sys::format!("{s}").ok().and_then(|sock_str| {
            ShortCStr::concat(&[&c"FDSHELL_SOCKET=".into(), &sock_str])
                .map(ExportedCStr::from)
                .ok()
        })
    });

    env_iter
        .chain(exports_iter)
        .chain(pid_entry)
        .chain(sock_entry)
        .collect()
}
