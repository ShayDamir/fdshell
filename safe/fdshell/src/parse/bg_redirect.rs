use crate::error::parse::{ParseError, report_error};
use crate::redirect::{RedirectDef, RedirectDirection, RedirectSource};
use error_stack::{Report, ResultExt, bail};
use sys::ShortCStr;

pub struct BgRedirectResult {
    pub redirects: Vec<RedirectDef>,
    pub pidvar: Option<ShortCStr>,
    pub bg_force: bool,
}

pub fn parse_bg_redirect(t: &ShortCStr) -> Result<Option<BgRedirectResult>, Report<ParseError>> {
    let b = t.as_bytes().change_context(ParseError::Reason {
        reason: "internal string state",
    })?;
    if !b.starts_with(b"&>") {
        return Ok(None);
    }
    let rest = t
        .strip_prefix(b"&>")
        .ok_or_else(|| report_error("internal string state", 0))?;
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
        RedirectSource::Var(
            path.get(1..)
                .ok_or_else(|| report_error("internal string state", 0))?,
        )
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
        Ok(_) => bail!(ParseError::Reason {
            reason: "duplicate redirect",
        }),
        Err(i) => {
            redirects.insert(i, r);
            Ok(())
        }
    }
}
