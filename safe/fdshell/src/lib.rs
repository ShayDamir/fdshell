#![forbid(unsafe_code)]

mod app;
mod capture;
mod caret;
mod case_exec;
mod cd;
mod child;
mod cli;
mod cmd_subst;
mod comment;
mod cond;
mod debug;
mod envfilter;
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
mod loop_control;
mod parse;
mod pipeline;
mod postlaunch;
mod redirect;
mod repl;
mod replacer;
mod run;
mod run_dispatch;
mod script;
mod segment;
mod state;
mod substitute;
mod task;

#[cfg(test)]
mod tests;

pub use app::AppError;
pub use cli::{CliArgs, load_script, parse_cli_args};
pub use debug::install_debug_hooks;
pub use init::{FdShellMode, init_shellfd};
pub use repl::{exec_cmd, run};
pub use state::ShellState;
