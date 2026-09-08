#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

extern crate alloc;

pub mod argparse;
pub mod copy_file_range;
pub mod error;
pub mod eventfd;
pub mod execat;
pub mod execfd;
pub mod fchmod;
pub mod memfd;
pub mod mkdirat;
pub mod openat2;
pub mod pipe;
pub mod renameat2;
pub mod resolve;
pub mod timerfd;
