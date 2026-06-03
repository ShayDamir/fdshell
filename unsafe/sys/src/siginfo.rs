#[repr(C)]
pub struct SigInfo {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    _pad_align: [u8; 4],
    pub si_pid: i32,
    pub si_uid: i32,
    pub si_status: i32,
    _rest: [u8; 100],
}

pub enum WaitStatus {
    Exited(i32),
    Signaled(i32),
}

impl WaitStatus {
    pub fn exit_code(&self) -> i32 {
        match self {
            WaitStatus::Exited(c) => *c,
            WaitStatus::Signaled(s) => 128 + *s,
        }
    }
}
