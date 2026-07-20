use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::{ExportedCStr, ExportedFd, ShortCStr};

use crate::envfilter::EnvFilter;

fn export_entry(k: &ShortCStr, v: &ShortCStr, env_filter: &EnvFilter) -> Option<ExportedCStr> {
    (!k.eq_bytes(b"FDSHELL_PID")).then_some(())?;
    (!k.eq_bytes(b"FDSHELL_SOCKET")).then_some(())?;
    env_filter.is_allowed(k).then_some(())?;
    Some(ExportedCStr::from(
        ShortCStr::concat(&[k, &c"=".into(), v]).ok()?,
    ))
}

pub(crate) fn get_environ(
    pid: i32,
    environ: &[(ShortCStr, ShortCStr)],
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    exec_sock: Option<&ExportedFd>,
) -> Vec<ExportedCStr> {
    let env_iter = environ
        .iter()
        .filter_map(|(k, v)| export_entry(k, v, env_filter));
    let exports_iter = exports
        .iter()
        .filter_map(|(k, v)| export_entry(k, v, env_filter));

    let pid_entry = sys::format!("FDSHELL_PID={pid}")
        .map(ExportedCStr::from)
        .ok();

    let sock_entry = exec_sock.and_then(|s| {
        sys::format!("FDSHELL_SOCKET={s}")
            .map(ExportedCStr::from)
            .ok()
    });

    env_iter
        .chain(exports_iter)
        .chain(pid_entry)
        .chain(sock_entry)
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;
