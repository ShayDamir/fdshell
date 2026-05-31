use core::sync::atomic::{AtomicU32, Ordering};

static CACHED_UMASK: AtomicU32 = AtomicU32::new(0);

pub fn init() {
    let mask = read_proc_umask().unwrap_or_else(|| {
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

fn read_proc_umask() -> Option<u32> {
    use crate::fcntl::O_RDONLY;
    let fd = crate::openat2::open(c"/proc/self/status", O_RDONLY).ok()?;
    let mut buf = [0u8; 4096];
    let n = crate::rw::read(&fd, &mut buf).ok()? as usize;
    drop(fd);
    let data = buf.get(..n)?;
    let start = data.windows(7).position(|w| w == b"Umask:\t")? + 7;
    let tail = data.get(start..)?;
    let len = tail.iter().position(|&b| b == b'\n')?;
    let s = core::str::from_utf8(tail.get(..len)?).ok()?;
    u32::from_str_radix(s, 8).ok()
}
