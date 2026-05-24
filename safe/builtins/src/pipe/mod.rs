use sys::fcntl::O_CLOEXEC;

pub mod parse;

pub fn pipe_exec(flags: i32) -> Result<(), i32> {
    let (rd, wr) = sys::pipe::pipe2(O_CLOEXEC | flags)?;
    sys::shellfd::send_fd(&rd, c"rd")?;
    sys::shellfd::send_fd(&wr, c"wr")
}
