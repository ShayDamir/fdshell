use super::*;
use crate::capture::Capture;
use crate::parse::CommandLine;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use sys::ShortCStr;

fn make_cmdline(args: &[&str]) -> CommandLine {
    let args_vec: Vec<ShortCStr> = args
        .iter()
        .map(|s| ShortCStr::from_vec(s.as_bytes().to_vec()).unwrap())
        .collect();
    CommandLine {
        builtin: false,
        command: ShortCStr::from_vec(b"envfilter".to_vec()).unwrap(),
        args: args_vec,
        args_fq: vec![false; args.len()],
        captures: vec![],
        redirects: vec![],
        pidvar: None,
        bg_force: false,
    }
}

fn make_cell() -> ForkCell<ShellState> {
    ForkCell::new(ShellState::new())
}

fn make_line(args: &[&str]) -> Vec<u8> {
    args.join(" ").into_bytes()
}

#[test]
fn help_returns_ok() {
    let line = make_line(&["envfilter", "--help"]);
    let cmdline = make_cmdline(&["--help"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn short_help_returns_ok() {
    let line = make_line(&["envfilter", "-h"]);
    let cmdline = make_cmdline(&["-h"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn clear_clears_filter() {
    let line = make_line(&["envfilter", "--allow", "PATH", "--clear"]);
    let cmdline = make_cmdline(&["--allow", "PATH", "--clear"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert!(state.env_filter.allow.is_empty());
    assert!(state.env_filter.deny.is_empty());
}

#[test]
fn allow_adds_patterns() {
    let line = make_line(&["envfilter", "--allow", "PATH", "HOME"]);
    let cmdline = make_cmdline(&["--allow", "PATH", "HOME"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert_eq!(state.env_filter.allow.len(), 2);
    assert!(state.env_filter.deny.is_empty());
}

#[test]
fn deny_adds_patterns() {
    let line = make_line(&["envfilter", "--deny", "*_KEY"]);
    let cmdline = make_cmdline(&["--deny", "*_KEY"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert!(state.env_filter.allow.is_empty());
    assert_eq!(state.env_filter.deny.len(), 1);
}

#[test]
fn allow_and_deny_both_work() {
    let line = make_line(&["envfilter", "--allow", "PATH", "--deny", "*_KEY"]);
    let cmdline = make_cmdline(&["--allow", "PATH", "--deny", "*_KEY"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert_eq!(state.env_filter.allow.len(), 1);
    assert_eq!(state.env_filter.deny.len(), 1);
}

#[test]
fn list_prints_rules() {
    let line = make_line(&["envfilter", "--allow", "PATH", "--list"]);
    let cmdline = make_cmdline(&["--allow", "PATH", "--list"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert_eq!(state.env_filter.allow.len(), 1);
}

#[test]
fn no_args_returns_error() {
    let line = make_line(&["envfilter"]);
    let cmdline = make_cmdline(&[]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::EnvfilterNoArgs
    ));
}

#[test]
fn unknown_flag_returns_error() {
    let line = make_line(&["envfilter", "--bogus"]);
    let cmdline = make_cmdline(&["--bogus"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::EnvfilterUnknownFlag
    ));
}

#[test]
fn allow_without_pattern_returns_error() {
    let line = make_line(&["envfilter", "--allow"]);
    let cmdline = make_cmdline(&["--allow"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::PatternRequired("--allow")
    ));
}

#[test]
fn deny_without_pattern_returns_error() {
    let line = make_line(&["envfilter", "--deny"]);
    let cmdline = make_cmdline(&["--deny"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::PatternRequired("--deny")
    ));
}

#[test]
fn allow_with_flag_as_pattern_returns_error() {
    let line = make_line(&["envfilter", "--allow", "--deny"]);
    let cmdline = make_cmdline(&["--allow", "--deny"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::PatternRequired("--allow")
    ));
}

#[test]
fn deny_with_flag_as_pattern_returns_error() {
    let line = make_line(&["envfilter", "--deny", "--allow"]);
    let cmdline = make_cmdline(&["--deny", "--allow"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::PatternRequired("--deny")
    ));
}

#[test]
fn captures_not_supported() {
    let line = make_line(&["envfilter", "--allow", "PATH"]);
    let mut cmdline = make_cmdline(&["--allow", "PATH"]);
    cmdline.captures = vec![Capture {
        var: ShortCStr::from_vec(b"fd".to_vec()).unwrap(),
        tag: None,
        force: false,
    }];
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::CapturesNotSupported { .. }
    ));
}

#[test]
fn redirects_not_supported() {
    let line = make_line(&["envfilter", "--allow", "PATH"]);
    let mut cmdline = make_cmdline(&["--allow", "PATH"]);
    cmdline.redirects = vec![RedirectDef {
        export_to: 1,
        direction: RedirectDirection::Write,
        source: RedirectSource::Var(ShortCStr::from_vec(b"test".to_vec()).unwrap()),
    }];
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::RedirectNotSupported { .. }
    ));
}

#[test]
fn builtin_keyword_not_supported() {
    let line = make_line(&["builtin", "envfilter", "--allow", "PATH"]);
    let mut cmdline = make_cmdline(&["--allow", "PATH"]);
    cmdline.builtin = true;
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_err());
    let report = result.unwrap_err();
    assert!(matches!(
        report.current_context(),
        CmdError::BuiltinKeywordNotSupported { .. }
    ));
}

#[test]
fn multiple_allow_patterns() {
    let line = make_line(&["envfilter", "--allow", "PATH", "HOME", "USER"]);
    let cmdline = make_cmdline(&["--allow", "PATH", "HOME", "USER"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert_eq!(state.env_filter.allow.len(), 3);
}

#[test]
fn multiple_deny_patterns() {
    let line = make_line(&["envfilter", "--deny", "*_KEY", "*_TOKEN", "*_SECRET"]);
    let cmdline = make_cmdline(&["--deny", "*_KEY", "*_TOKEN", "*_SECRET"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert_eq!(state.env_filter.deny.len(), 3);
}

#[test]
fn list_with_no_rules_prints_nothing() {
    let line = make_line(&["envfilter", "--list"]);
    let cmdline = make_cmdline(&["--list"]);
    let cell = make_cell();
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert!(state.env_filter.allow.is_empty());
    assert!(state.env_filter.deny.is_empty());
}

#[test]
fn clear_resets_all_rules() {
    // First add some rules via state manipulation, then clear
    let cell = make_cell();
    {
        let mut state = cell.borrow_mut().unwrap();
        state
            .env_filter
            .allow
            .push(ShortCStr::from_vec(b"PATH".to_vec()).unwrap());
        state
            .env_filter
            .deny
            .push(ShortCStr::from_vec(b"*_KEY".to_vec()).unwrap());
    }
    let line = make_line(&["envfilter", "--clear"]);
    let cmdline = make_cmdline(&["--clear"]);
    let result = run_envfilter(&line, &cmdline, &cell);
    assert!(result.is_ok());
    let state = cell.borrow().unwrap();
    assert!(state.env_filter.allow.is_empty());
    assert!(state.env_filter.deny.is_empty());
}
