use crate::error::parse::ParseError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::ShortCStr;

pub struct BgRedirectResult {
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

pub fn parse_bg_redirect(
    t: &ShortCStr,
    mask: &[bool],
) -> Result<Option<BgRedirectResult>, Report<ParseError>> {
    let Some(rest) = t.strip_prefix(b"&>") else {
        return Ok(None);
    };
    if let Some(name) = rest.strip_prefix(b"|&") {
        return Ok(Some(BgRedirectResult {
            redirects: Vec::new(),
            pidvar: Some(name),
            bg_force: true,
        }));
    }
    if let Some(name) = rest.strip_prefix(b"&") {
        return Ok(Some(BgRedirectResult {
            redirects: Vec::new(),
            pidvar: Some(name),
            bg_force: false,
        }));
    }
    let rest_len = rest.len();
    let (path, direction) = if let Some(p) = rest.strip_prefix(b">") {
        (p, RedirectDirection::Append)
    } else {
        (rest, RedirectDirection::Write)
    };
    let source = if path.starts_with(b"%") {
        RedirectSource::Var(path.get(1..).ok_or(ParseError::Never)?)
    } else {
        // The path is the token suffix after `&>` (and `>`); align its mask.
        let path_offset = 2 + (rest_len - path.len());
        let path_mask = mask.get(path_offset..).unwrap_or(&[]).to_vec();
        RedirectSource::path(path, path_mask)
    };
    let r1 = RedirectDef {
        export_to: 1,
        direction,
        source: source.clone(),
    };
    let r2 = RedirectDef {
        export_to: 2,
        direction,
        source,
    };
    Ok(Some(BgRedirectResult {
        redirects: vec![r1, r2],
        pidvar: None,
        bg_force: false,
    }))
}

/// Insert `r` keeping the list sorted by target fd, and the entries of one
/// target fd in command-line order. POSIX #2.5 applies every redirection of a
/// command in the order written, so a second redirect to a fd is not an error:
/// the last one to that fd wins, and the earlier ones still take effect (their
/// files are opened/truncated, and a failing earlier open aborts the command).
/// `partition_point` is the stable insertion point; `binary_search_by_key`
/// returns an arbitrary equal-key index, which cannot express "last wins".
pub fn insert_redirect(redirects: &mut Vec<RedirectDef>, r: RedirectDef) {
    let at = redirects.partition_point(|x| x.export_to <= r.export_to);
    redirects.insert(at, r);
}
