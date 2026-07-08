use error_stack::{Report, ResultExt};
use std::io::Write as _;

use crate::error::cmd::CmdError;
use crate::error::read::ReadError;
use crate::parse::CommandLine;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;
use sys::siginfo::WaitStatus;

use super::validation::*;

use collect::collect_targets;
use flags::SourceFd;
use flags::parse_flags;
use io::read_line;
use strip::strip_prefix;
use words::split_fields;

pub(crate) fn run_read(
    line: &[u8],
    cmdline: &CommandLine,
    cell: &ForkCell<ShellState>,
) -> Result<bool, Report<CmdError>> {
    check_builtin_not_supported(line, "read", cmdline.builtin)?;
    check_captures_not_supported(line, "read", &cmdline.captures)?;
    check_redirects_not_supported(line, "read", &cmdline.redirects)?;

    let (fd_source, max_bytes, prompt) = parse_flags(&cmdline.args)?;
    let targets = collect_targets(&cmdline.args)?;

    let prompt_text = prompt.unwrap_or(b"");
    if !prompt_text.is_empty() {
        std::io::stderr()
            .write_all(prompt_text)
            .change_context(ReadError::Io)
            .change_context(CmdError::Read)?;
    }

    let resolved_fd: Option<sys::LocalFd> = match &fd_source {
        SourceFd::FdVar(name) => {
            let state = cell.borrow().change_context(CmdError::Read)?;
            let var = ShortCStr::from_vec(name.clone())
                .change_context(ReadError::BadTarget)
                .change_context(CmdError::Read)?;
            Some(
                state
                    .fds
                    .get(&var)
                    .ok_or(ReadError::VarNotFound)
                    .change_context(CmdError::Read)?
                    .try_clone()
                    .change_context(CmdError::Read)?,
            )
        }
        _ => None,
    };

    let (data, eof) = read_line(&fd_source, resolved_fd.as_ref(), max_bytes)?;
    if data.is_empty() && eof {
        let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
        state.last_status = WaitStatus::Exited(1);
        return Ok(true);
    }

    let fields = split_fields(&data, targets.len());

    let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
    for (i, name) in targets.iter().enumerate() {
        let field = fields.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
        let var_name = strip_prefix(name);
        let s = ShortCStr::from_vec(field.to_vec())
            .change_context(ReadError::NulByte)
            .change_context(CmdError::Read)?;
        state.strings.insert(var_name, s);
    }
    state.last_status = WaitStatus::Exited(0);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_fields_single() {
        let fields = split_fields(b"hello world", 1);
        assert_eq!(fields, vec![b"hello world".to_vec()]);
    }

    #[test]
    fn test_split_fields_two_exact() {
        let fields = split_fields(b"hello world", 2);
        assert_eq!(fields, vec![b"hello".to_vec(), b"world".to_vec()]);
    }

    #[test]
    fn test_split_fields_two_extra() {
        let fields = split_fields(b"a b c d", 2);
        assert_eq!(fields, vec![b"a".to_vec(), b"b c d".to_vec()]);
    }

    #[test]
    fn test_split_fields_two_few() {
        let fields = split_fields(b"hello", 3);
        assert_eq!(fields, vec![b"hello".to_vec(), Vec::new(), Vec::new()]);
    }

    #[test]
    fn test_split_fields_tabs() {
        let fields = split_fields(b"a\tb\tc", 3);
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn test_split_fields_leading_spaces() {
        let fields = split_fields(b"  a  b  ", 3);
        assert_eq!(fields, vec![b"a".to_vec(), b"b".to_vec(), Vec::new()]);
    }

    #[test]
    fn test_strip_prefix_dollar() {
        let name = c"$FOO".into();
        assert_eq!(strip_prefix(&name), c"FOO".into());
    }

    #[test]
    fn test_strip_prefix_bare() {
        let name = c"FOO".into();
        assert_eq!(strip_prefix(&name), c"FOO".into());
    }

    #[test]
    fn test_no_targets_error() {
        let args: Vec<ShortCStr> = vec![];
        let result = collect_targets(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_fdvar_target_rejected() {
        let args = vec![c"%myvar".into()];
        let result = collect_targets(&args);
        assert!(result.is_err());
    }
}

mod collect;
mod flags;
mod io;
mod read_from_fd;
mod strip;
mod words;
