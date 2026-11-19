#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

extern crate alloc;

// The per-builtin `X_exec(&parse::XConfig, ..)` entry points stay free functions
// (task #601): each has exactly one caller — the `child/delegated.rs` dispatch
// table — so the family is a single-caller shape, not a reusable method on one
// builtin's parse type. `argparse.rs` records the `&CStr`-receiver half.

pub mod argparse;
pub mod copy_file_range;
pub mod error;
pub mod eventfd;
pub mod execat;
pub mod execfd;
pub mod fchmod;
pub mod memfd;
pub mod mkdirat;
pub mod mkfifoat;
pub mod mode;
pub mod openat2;
pub mod pipe;
pub mod renameat2;
pub mod resolve;
pub mod strict;
pub mod symlinkat;
pub mod timerfd;
pub mod unlinkat;
pub mod utimensat;

#[cfg(test)]
extern crate std;
