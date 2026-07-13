use std::collections::HashMap;
use std::ffi::CString;

use sys::ExportedFd;
use sys::ShortCStr;

use crate::envfilter::EnvFilter;

pub(crate) fn get_environ(
    cookie: &[u8],
    exports: &HashMap<ShortCStr, Vec<u8>>,
    env_filter: &EnvFilter,
    exec_sock: Option<&ExportedFd>,
) -> Vec<CString> {
    let env_iter = std::env::vars()
        .filter(|(k, _)| k != "FDSHELL_PID" && k != "FDSHELL_SOCKET")
        .filter(|(k, _)| env_filter.is_allowed(k.as_bytes()))
        .filter_map(|(k, v)| CString::new(format!("{k}={v}")).ok());
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
