#![forbid(unsafe_code)]

mod app;
mod capture;
mod caret;
mod cd;
mod child;
mod cmd_subst;
mod comment;
mod cond;
mod debug;
mod error;
mod exec;
mod expand;
mod exports;
mod for_run;
mod if_exec;
mod init;
mod intercept;
mod keywords;
mod launch;
mod loop_;
mod parse;
mod pipeline;
mod postlaunch;
mod redirect;
mod repl;
mod replacer;
mod resolve;
mod run;
mod script;
mod state;
mod substitute;
mod task;

#[cfg(test)]
mod tests;

pub use app::AppError;
pub use debug::install_debug_hooks;
pub use init::{FdShellMode, init_shellfd};
pub use repl::{exec_cmd, run};
pub use state::ShellState;
