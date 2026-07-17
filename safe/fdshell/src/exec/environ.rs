use alloc::ffi::CString;
use alloc::format;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ExportedFd;
use sys::RefCStr;
use sys::ShortCStr;

use crate::envfilter::EnvFilter;

pub(crate) fn get_environ(
    cookie: &[u8],
    exports: &HashMap<ShortCStr, ShortCStr>,
    env_filter: &EnvFilter,
    exec_sock: Option<&ExportedFd>,
) -> Vec<CString> {
    let env_iter = sys::env::environ_snapshot()
        .into_iter()
        .filter(|(k, _)| !k.eq_bytes(b"FDSHELL_PID") && !k.eq_bytes(b"FDSHELL_SOCKET"))
        .filter(|(k, _)| env_filter.is_allowed(k.as_bytes().unwrap_or(&[])))
        .filter_map(|(k, v)| {
            let key = k.as_bytes().ok()?;
            let val = v.as_bytes().ok()?;
            CString::new([key, b"=", val].concat()).ok()
        });
    let exports_iter = exports.iter().filter_map(|(k, v)| {
        if let Ok(key) = k.as_bytes() {
            if !env_filter.is_allowed(key) {
                return None;
            }
            let ref_cstr: RefCStr = v.clone().into();
            CString::new([key, b"=", ref_cstr.as_ref().to_bytes_with_nul()].concat()).ok()
        } else {
            None
        }
    });
    let pid_entry = [b"FDSHELL_PID=", cookie].concat();
    let pid_cstr = CString::new(pid_entry).ok();
    let sock_entry = exec_sock.map(|s| format!("FDSHELL_SOCKET={s}"));

    let mut result = env_iter.chain(exports_iter).collect::<Vec<_>>();
    if let Some(s) = pid_cstr {
        result.push(s);
    }
    if let Some(sock_str) = sock_entry
        && let Ok(sock_cstr) = CString::new(sock_str)
    {
        result.push(sock_cstr);
    }
    result
}
