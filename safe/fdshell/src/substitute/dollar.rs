#![forbid(unsafe_code)]

use sys::ShortCStr;

use crate::error::resolve::ResolveError;
use crate::state::ShellState;

pub(crate) fn dollar_subst(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut Vec<u8>,
) -> Result<(), ResolveError> {
    match peek.peek().copied() {
        Some(b'$') => {
            peek.next();
            let s = format!("{}", state.shell_pid);
            out.extend_from_slice(s.as_bytes());
        }
        Some(b'!') => {
            peek.next();
            if let Some(pid) = state.last_bg_pid {
                let s = format!("{}", pid);
                out.extend_from_slice(s.as_bytes());
            }
        }
        Some(b'{') => handle_brace(peek, state, out)?,
        Some(c) if c.is_ascii_alphanumeric() || c == b'_' => {
            let name_scs = super::percent::collect_name(peek)?;
            match state.strings.get(&name_scs) {
                Some(val) => {
                    out.extend_from_slice(val.as_bytes().map_err(|_| ResolveError::RefNotFound)?);
                }
                None => {
                    out.push(b'$');
                    out.extend_from_slice(
                        name_scs.as_bytes().map_err(|_| ResolveError::RefNotFound)?,
                    );
                }
            }
        }
        Some(b'?') => {
            peek.next();
            let code = state.last_status.exit_code();
            let s = format!("{}", code);
            out.extend_from_slice(s.as_bytes());
        }
        _ => out.push(b'$'),
    }
    Ok(())
}

fn handle_brace(
    peek: &mut std::iter::Peekable<impl Iterator<Item = u8>>,
    state: &ShellState,
    out: &mut Vec<u8>,
) -> Result<(), ResolveError> {
    peek.next();
    let mut name = Vec::new();
    let mut closed = false;
    while let Some(&nc) = peek.peek() {
        if nc == b'}' {
            closed = true;
            peek.next();
            break;
        }
        name.push(nc);
        peek.next();
    }
    if closed {
        let name_scs = ShortCStr::from_vec(name).map_err(|_| ResolveError::TokenTooLong)?;
        match state.strings.get(&name_scs) {
            Some(val) => {
                out.extend_from_slice(val.as_bytes().map_err(|_| ResolveError::RefNotFound)?)
            }
            None => {
                out.push(b'$');
                out.push(b'{');
                out.extend_from_slice(name_scs.as_bytes().map_err(|_| ResolveError::RefNotFound)?);
                out.push(b'}');
            }
        }
    } else {
        out.push(b'$');
        out.push(b'{');
        out.extend_from_slice(&name);
    }
    Ok(())
}
