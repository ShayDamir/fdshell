use alloc::ffi::CString;
use alloc::format;
use alloc::vec::Vec;
use hashbrown::HashMap;

use sys::ExportedFd;
use sys::ShortCStr;

use crate::envfilter::EnvFilter;

pub(crate) fn get_environ(
    cookie: &[u8],
    exports: &HashMap<ShortCStr, Vec<u8>>,
    env_filter: &EnvFilter,
    exec_sock: Option<&ExportedFd>,
) -> Vec<CString> {
    let env_iter = sys::env::environ_snapshot()
        .into_iter()
        .filter(|(k, _)| k != b"FDSHELL_PID" && k != b"FDSHELL_SOCKET")
        .filter(|(k, _)| env_filter.is_allowed(k))
        .filter_map(|(k, v)| CString::new([k.as_slice(), b"=", v.as_slice()].concat()).ok());
    let exports_iter = exports.iter().filter_map(|(k, v)| {
        if let Ok(key) = k.as_bytes() {
            if !env_filter.is_allowed(key) {
                return None;
            }
            CString::new([key, b"=", v.as_slice()].concat()).ok()
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
