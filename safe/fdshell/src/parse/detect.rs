use crate::error::parse::ParseError;
use crate::parse::detect_keyword;
use crate::parse::line::ParsedLine;
use error_stack::Report;
use sys::ShortCStr;

pub(crate) fn detect(
    tokens: &[(ShortCStr, usize, bool)],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let first = match tokens.first() {
        Some((t, _, _)) => t,
        None => return Ok(None),
    };

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && let Some(var) = lhs.strip_prefix(b"%")
        && let Some(value) = rhs.strip_prefix(b"%")
    {
        return Ok(Some(ParsedLine::AssignFd { var, value }));
    }

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && !lhs.is_empty()
        && !lhs.starts_with(b"%")
    {
        return Ok(Some(ParsedLine::AssignStr {
            var: lhs,
            value: rhs,
        }));
    }

    if first.eq_bytes(b"unset") {
        return detect_keyword::detect_unset(tokens);
    }

    if first.eq_bytes(b"umask") {
        return detect_keyword::detect_umask(tokens);
    }

    detect_keyword::detect_control(tokens)
}
