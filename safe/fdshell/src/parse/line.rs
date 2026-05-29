use crate::parse::{CommandLine, Pipeline};
use sys::ShortCStr;
use sys::errno::EINVAL;

pub enum ParsedLine {
    Cmd(CommandLine),
    Pipeline(Pipeline),
    Assign { var: ShortCStr, value: ShortCStr },
    Unset(ShortCStr),
}

pub(crate) fn detect(tokens: &[ShortCStr]) -> Result<Option<ParsedLine>, i32> {
    let first = match tokens.first() {
        Some(t) => t,
        None => return Ok(None),
    };

    if let Some((lhs, rhs)) = first.split_once_byte(b'=')
        && let Some(var) = lhs.strip_prefix(b"%")
        && let Some(value) = rhs.strip_prefix(b"%")
    {
        return Ok(Some(ParsedLine::Assign { var, value }));
    }

    if first.as_bytes() == b"unset" {
        let target = tokens.get(1).ok_or(EINVAL)?;
        if let Some(var) = target.strip_prefix(b"%") {
            return Ok(Some(ParsedLine::Unset(var)));
        }
        return Err(EINVAL);
    }

    Ok(None)
}
