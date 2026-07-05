use builtins::error::Suggestion;
use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

use super::validation::*;

pub(crate) fn run_envfilter(
    line: &[u8],
    cmdline: &crate::parse::CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_builtin_not_supported(line, "envfilter", cmdline.builtin)?;
    check_captures_not_supported(line, "envfilter", &cmdline.captures)?;
    check_redirects_not_supported(line, "envfilter", &cmdline.redirects)?;

    if cmdline.args.is_empty() {
        return Err(
            err_at(line, 0, CmdError::EnvfilterNoArgs).attach_opaque(Suggestion(
                "valid flags: --allow, --deny, --list, --clear, --help",
            )),
        );
    }

    let mut do_list = false;
    let mut allow_patterns = Vec::new();
    let mut deny_patterns = Vec::new();

    let mut i = 0;
    while let Some(arg) = cmdline.args.get(i) {
        let arg_bytes = arg.as_bytes().change_context(CmdError::Exec)?;

        if arg_bytes == b"--help" || arg_bytes == b"-h" {
            print_help();
            return Ok(true);
        } else if arg_bytes == b"--clear" {
            let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
            state.env_filter.clear();
            return Ok(true);
        } else if arg_bytes == b"--list" {
            do_list = true;
            i += 1;
            continue;
        } else if arg_bytes == b"--allow" {
            i += 1;
            match cmdline.args.get(i) {
                None => {
                    let pos = find_arg_pos(line, &cmdline.args, i - 1);
                    return Err(err_at(line, pos, CmdError::PatternRequired("--allow")));
                }
                Some(next) => {
                    if next.as_bytes().unwrap_or(&[]).starts_with(b"--") {
                        let pos = find_arg_pos(line, &cmdline.args, i - 1);
                        return Err(err_at(line, pos, CmdError::PatternRequired("--allow")));
                    }
                }
            }
            while let Some(next) = cmdline.args.get(i) {
                if next.as_bytes().unwrap_or(&[]).starts_with(b"--") {
                    break;
                }
                allow_patterns.push(next.clone());
                i += 1;
            }
            continue;
        } else if arg_bytes == b"--deny" {
            i += 1;
            match cmdline.args.get(i) {
                None => {
                    let pos = find_arg_pos(line, &cmdline.args, i - 1);
                    return Err(err_at(line, pos, CmdError::PatternRequired("--deny")));
                }
                Some(next) => {
                    if next.as_bytes().unwrap_or(&[]).starts_with(b"--") {
                        let pos = find_arg_pos(line, &cmdline.args, i - 1);
                        return Err(err_at(line, pos, CmdError::PatternRequired("--deny")));
                    }
                }
            }
            while let Some(next) = cmdline.args.get(i) {
                if next.as_bytes().unwrap_or(&[]).starts_with(b"--") {
                    break;
                }
                deny_patterns.push(next.clone());
                i += 1;
            }
            continue;
        } else {
            let pos = find_arg_pos(line, &cmdline.args, i);
            return Err(
                err_at(line, pos, CmdError::EnvfilterUnknownFlag).attach_opaque(Suggestion(
                    "valid flags: --allow, --deny, --list, --clear, --help",
                )),
            );
        }
    }

    if !allow_patterns.is_empty() || !deny_patterns.is_empty() {
        let mut state = cell.borrow_mut().change_context(CmdError::Exec)?;
        state.env_filter.allow.extend(allow_patterns);
        state.env_filter.deny.extend(deny_patterns);
    }

    if do_list {
        let state = cell.borrow().change_context(CmdError::Exec)?;
        print_rules(&state.env_filter);
    }

    Ok(true)
}

fn find_arg_pos(line: &[u8], args: &[ShortCStr], idx: usize) -> usize {
    args.get(idx)
        .and_then(|a| a.as_bytes().ok())
        .and_then(|bytes| line.windows(bytes.len()).position(|w| w == bytes))
        .unwrap_or(0)
}

fn print_help() {
    println!("Usage: envfilter [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --allow <pattern>...   Add allowlist glob patterns");
    println!("  --deny <pattern>...    Add denylist glob patterns");
    println!("  --list                 Show current rules");
    println!("  --clear                Clear all rules");
    println!("  --help, -h             Show this help");
    println!();
    println!("Patterns support * wildcard only.");
    println!("Allowlist is applied first, then denylist removes from it.");
}

fn print_rules(filter: &crate::envfilter::EnvFilter) {
    let allow_strs: Vec<&str> = filter
        .allow
        .iter()
        .filter_map(|s| core::str::from_utf8(s.as_bytes().unwrap_or(&[])).ok())
        .collect();
    let deny_strs: Vec<&str> = filter
        .deny
        .iter()
        .filter_map(|s| core::str::from_utf8(s.as_bytes().unwrap_or(&[])).ok())
        .collect();

    if !allow_strs.is_empty() {
        println!("allow: {}", allow_strs.join(" "));
    }
    if !deny_strs.is_empty() {
        println!("deny: {}", deny_strs.join(" "));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::capture::Capture;
    use crate::parse::CommandLine;
    use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};

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
}
