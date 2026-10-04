use super::Token;
use crate::error::parse::ParseError;
use crate::parse::array_ref;
use crate::parse::detect_keyword;
use crate::parse::envassign;
use crate::parse::line::ParsedLine;
use error_stack::Report;
use sys::ShortCStr;

pub(crate) fn detect(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some((t, _, _, _, _)) => t,
        None => return Ok(None),
    };

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && let Some(var) = lhs.strip_prefix(b"%")
        && let Some(line) = fd_assign(var, rhs)
    {
        return Ok(Some(line));
    }

    if envassign::is_env_assign(first) {
        return detect_assigns(tokens);
    }

    if first.eq_bytes(b"unset") {
        return detect_keyword::detect_unset(tokens);
    }

    if first.eq_bytes(b"umask") {
        return detect_keyword::detect_umask(tokens);
    }

    detect_keyword::detect_control(tokens)
}

/// A statement of `NAME=value` words: one word is a persistent `AssignStr`,
/// several an `AssignStrs`; a non-assignment word following the assignments
/// is a command with a scoped prefix — not an assignment statement.
fn detect_assigns(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let assigns = match envassign::collect_all(tokens) {
        Some(assigns) => assigns,
        None => return Ok(None),
    };
    match assigns.len() {
        1 => {
            let (var, value) = assigns.into_iter().next().ok_or(ParseError::Never)?;
            Ok(Some(ParsedLine::AssignStr { var, value }))
        }
        _ => Ok(Some(ParsedLine::AssignStrs { assigns })),
    }
}

/// `%var=%name` fd copy, `%var=%arr[N]` indexed read-out, `%var=[]` empty
/// array, `%var+=%name` array append; `None` when the token is not an fd assign.
fn fd_assign(var: ShortCStr, rhs: ShortCStr) -> Option<ParsedLine> {
    let Some(value) = rhs.strip_prefix(b"%") else {
        return rhs
            .eq_bytes(b"[]")
            .then_some(ParsedLine::AssignArrayEmpty { var });
    };
    if let Some(base) = var.strip_suffix(b"+") {
        return if base.is_empty() {
            None
        } else {
            Some(ParsedLine::AppendFd { var: base, value })
        };
    }
    if let Some((arr, index)) = array_ref::split_index_ref(&value) {
        return Some(ParsedLine::AssignFdIndex {
            var,
            value: arr,
            index,
        });
    }
    Some(ParsedLine::AssignFd { var, value })
}
