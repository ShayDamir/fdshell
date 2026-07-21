use core::sync::atomic::{AtomicU32, Ordering};
use error_stack::{Report, ResultExt};

/// Failure to read umask from `/proc/self/status`.
#[derive(Debug, displaydoc::Display)]
pub enum UmaskError {
    /// failed to open /proc/self/status
    ProcOpen,
    /// failed to read /proc/self/status
    ProcRead,
    /// no Umask line found in /proc/self/status
    UmaskNotFound,
    /// invalid octal umask value in /proc/self/status
    InvalidUmask,
    /// impossible
    Never,
}

impl core::error::Error for UmaskError {}

static CACHED_UMASK: AtomicU32 = AtomicU32::new(0);

pub fn init() {
    let mask = read_proc_umask().unwrap_or_else(|_| {
        // TODO: log::info!("failed to read umask from /proc/self/status, falling back to umask(0)")
        // SAFETY: `umask` always succeeds; 0 is never a permanent mask.
        let old = unsafe { libc::umask(0) };
        // SAFETY: `old` is the previous umask — valid restore value.
        unsafe { libc::umask(old) };
        old as u32
    });
    CACHED_UMASK.store(mask & 0o777, Ordering::Relaxed);
}

pub fn get() -> u32 {
    CACHED_UMASK.load(Ordering::Relaxed)
}

pub fn set(mask: u32) -> u32 {
    let cached = CACHED_UMASK.load(Ordering::Relaxed);
    // SAFETY: `umask` always succeeds; any u32 is a valid `mode_t`.
    let old = unsafe { libc::umask(mask as libc::mode_t) as u32 };
    debug_assert_eq!(old & 0o777, cached, "umask cache desync");
    CACHED_UMASK.store(mask & 0o777, Ordering::Relaxed);
    old
}

fn read_proc_umask() -> Result<u32, Report<UmaskError>> {
    use crate::fcntl::O_RDONLY;
    let fd = crate::openat2::open(c"/proc/self/status", O_RDONLY)
        .change_context(UmaskError::ProcOpen)?;
    let mut buf = [0u8; 4096];
    let n = crate::rw::read(&fd, &mut buf).change_context(UmaskError::ProcRead)?;
    let data = buf.get(..n).ok_or(UmaskError::Never)?;
    let (_, tail) = crate::split::split_once(data, b"Umask:\t").ok_or(UmaskError::UmaskNotFound)?;
    let (s, _) = crate::split::split_once(tail, b"\n").ok_or(UmaskError::InvalidUmask)?;
    let s = core::str::from_utf8(s).change_context(UmaskError::InvalidUmask)?;
    u32::from_str_radix(s, 8).change_context(UmaskError::InvalidUmask)
}
