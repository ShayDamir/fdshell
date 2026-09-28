//! The builtin dispatch table: name → handler, plus the shared `Handler`
//! type. Kept in its own file to hold `dispatch.rs` under the line budget.

use builtins::error::BuiltinError;
use error_stack::Report;

use crate::child::Ctx;

use crate::child::accept;
use crate::child::bind;
use crate::child::copy_file_range;
use crate::child::delegated;
use crate::child::exec_fd;
use crate::child::explain;
use crate::child::fdexplain;
use crate::child::fdops;
use crate::child::flock;
use crate::child::listen;
use crate::child::ls;
use crate::child::printf;
use crate::child::readlink;
use crate::child::resolve;
use crate::child::simple;
use crate::child::statx;
use crate::child::test;
use crate::child::type_cmd;
use crate::child::verity;

pub(crate) type Handler = fn(&Ctx) -> Result<i32, Report<BuiltinError>>;

pub(crate) const DISPATCH: &[(&[u8], Handler)] = &[
    (b"true", simple::handle_true),
    (b"false", simple::handle_false),
    (b"help", simple::handle_help),
    (b"pwd", simple::handle_pwd),
    (b"fchmod", delegated::handle_fchmod),
    (b"echo", simple::handle_echo),
    (b"copy_file_range", copy_file_range::handle_copy_file_range),
    (b"explain", explain::handle_explain),
    (b"fdexplain", fdexplain::handle_fdexplain),
    (b"pipe", delegated::handle_pipe),
    (b"mkdirat", delegated::handle_mkdirat),
    (b"memfd", delegated::handle_memfd),
    (b"openat2", delegated::handle_openat2),
    (b"printf", printf::handle_printf),
    (b"renameat2", delegated::handle_renameat2),
    (b"timerfd", delegated::handle_timerfd),
    (b"eventfd", delegated::handle_eventfd),
    (b"fsync", fdops::handle_fsync),
    (b"ftruncate", fdops::handle_ftruncate),
    (b"lseek", fdops::handle_lseek),
    (b"fallocate", fdops::handle_fallocate),
    (b"flock", flock::handle_flock),
    (b"ls", ls::handle_ls),
    (b"statx", statx::handle_statx),
    (b"exec_fd", exec_fd::handle_exec_fd),
    (b"exec_at", exec_fd::handle_exec_at),
    (b"readlink", readlink::handle_readlink),
    (b"resolve", resolve::handle_resolve),
    (b"test", test::handle_test),
    (b"[", test::handle_test),
    (b"type", type_cmd::handle_type),
    (b"verity", verity::handle_verity),
    (b"bind", bind::handle_bind),
    (b"listen", listen::handle_listen),
    (b"accept", accept::handle_accept),
];
