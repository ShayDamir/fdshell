use crate::parse::CommandLine;
use std::ffi::CString;
use sys::errno::EINVAL;

pub enum ParsedLine {
    Cmd(CommandLine),
    Assign { var: CString, value: CString },
    Unset(CString),
}

pub(crate) fn detect(tokens: &[Vec<u8>]) -> Result<Option<ParsedLine>, i32> {
    let first = match tokens.first() {
        Some(t) => t.as_slice(),
        None => return Ok(None),
    };

    if let Some(pos) = first.iter().position(|&b| b == b'=') {
        let lhs = first.get(..pos).ok_or(EINVAL)?;
        let rhs = first.get(pos + 1..).ok_or(EINVAL)?;
        if lhs.starts_with(b"%") && rhs.starts_with(b"%") {
            let var = CString::new(lhs.get(1..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
            let value = CString::new(rhs.get(1..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
            return Ok(Some(ParsedLine::Assign { var, value }));
        }
    }

    if first == b"unset" {
        let target = tokens.get(1).ok_or(EINVAL)?.as_slice();
        if target.starts_with(b"%") {
            let var = CString::new(target.get(1..).ok_or(EINVAL)?).map_err(|_| EINVAL)?;
            return Ok(Some(ParsedLine::Unset(var)));
        }
        return Err(EINVAL);
    }

    Ok(None)
}
