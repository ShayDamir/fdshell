use crate::error::parse::ParseError;
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use alloc::vec;
use alloc::vec::Vec;
use error_stack::{Report, bail};
use sys::ShortCStr;

pub struct BgRedirectResult {
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

pub fn parse_bg_redirect(t: &ShortCStr) -> Result<Option<BgRedirectResult>, Report<ParseError>> {
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
    let (path, direction) = if let Some(p) = rest.strip_prefix(b">") {
        (p, RedirectDirection::Append)
    } else {
        (rest, RedirectDirection::Write)
    };
    let source = if path.starts_with(b"%") {
        RedirectSource::Var(path.get(1..).ok_or(ParseError::Never)?)
    } else {
        RedirectSource::path(path)
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

pub fn insert_redirect(
    redirects: &mut Vec<RedirectDef>,
    r: RedirectDef,
) -> Result<(), Report<ParseError>> {
    match redirects.binary_search_by_key(&r.export_to, |x| x.export_to) {
        Ok(_) => bail!(ParseError::DuplicateRedirect),
        Err(i) => {
            redirects.insert(i, r);
            Ok(())
        }
    }
}
