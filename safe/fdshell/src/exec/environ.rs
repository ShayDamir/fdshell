#![forbid(unsafe_code)]

use std::ffi::CString;

use sys::ShortCStr;

pub(crate) fn get_environ(cookie: &[u8], exports: &[(ShortCStr, Vec<u8>)]) -> Vec<CString> {
    let env_iter = std::env::vars()
        .filter(|(k, _)| k != "FDSHELL_CAPTURE")
        .filter_map(|(k, v)| CString::new(format!("{k}={v}")).ok());
    let exports_iter = exports.iter().filter_map(|(k, v)| {
        if let Ok(key) = k.as_bytes() {
            CString::new([key, b"=", v.as_slice()].concat()).ok()
        } else {
            None
        }
    });
    if sys::shellfd::capture_active() {
        let entry = [b"FDSHELL_CAPTURE=", cookie].concat();
        env_iter
            .chain(exports_iter)
            .chain(CString::new(entry).ok())
            .collect()
    } else {
        env_iter.chain(exports_iter).collect()
    }
}
