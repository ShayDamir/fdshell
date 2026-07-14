/// Exit the current process immediately with the given status code.
///
/// Unlike `std::process::exit`, this does not run atexit handlers,
/// flush stdio buffers, or trigger any cleanup — it calls `_exit`
/// directly, making it safe to invoke from multithreaded test processes.
pub fn exit(code: i32) -> ! {
    // SAFETY: `_exit` is a valid libc function that never returns.
    // It takes a single i32 status code and terminates the process.
    unsafe { libc::_exit(code) }
}
