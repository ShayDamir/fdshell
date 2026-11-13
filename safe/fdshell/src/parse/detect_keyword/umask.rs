// mutants: -C --lib -C --test -C umask -- -E 'test(/^parse::detect_keyword::/) | binary(umask)'

//! `umask`'s option and operand scan (task #554), moved out of `detect_keyword.rs`
//! because the keyword now has two grammars for its operand and an option surface.
//!
//! Options come first, as XBD puts them, and the scan copies bash's
//! `internal_getopt (list, "Sp")`: a word that starts with `-` and is longer than
//! one byte is a flag cluster of `p` and `S` — clusters and repeats are fine, any
//! other letter is the error — and a lone `--` ends the option scanning. A bare `-`
//! is an **operand**, the symbolic no-op whose operator is that `-`. Scanning stops
//! at the first operand, so a flag written after one is a second operand, and a
//! second operand stays a parse error here where bash ignores it (task #472).

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::line::ParsedLine;
use crate::parse::umask_mode::UmaskMode;
use error_stack::{Report, ResultExt, bail, ensure};
use sys::ShortCStr;

/// Parse the statement; `tokens[0]` is the keyword itself.
pub(crate) fn detect(tokens: &[Token]) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let mut flags = Flags {
        dash_p: false,
        dash_s: false,
    };
    let mut mode: Option<UmaskMode> = None;
    let mut options = true;
    for Token { word, .. } in tokens.iter().skip(1) {
        if options && word.eq_bytes(b"--") {
            options = false;
            continue;
        }
        if options && word.len() > 1 && word.starts_with(b"-") {
            flags.add(word)?;
            continue;
        }
        ensure!(mode.is_none(), ParseError::UmaskTakesAtMostOneArgument);
        mode = Some(UmaskMode::parse(word)?);
        options = false;
    }
    Ok(Some(ParsedLine::Umask {
        mode,
        dash_p: flags.dash_p,
        dash_s: flags.dash_s,
    }))
}

/// The two options bash's getopt accepts; both are booleans, so `-pS`, `-Sp` and
/// `-SS` all read the same as their separate spellings.
struct Flags {
    dash_p: bool,
    dash_s: bool,
}

impl Flags {
    /// One cluster: every byte after the `-` is an option letter, and the first
    /// byte that is not is reported on the whole word. bash reports the same letter
    /// as a usage error at rc 2; a fdshell parse error is rc 1 (the rc-1-vs-2 family).
    fn add(&mut self, word: &ShortCStr) -> Result<(), Report<ParseError>> {
        let bytes = word.as_bytes().change_context(ParseError::Never)?;
        for byte in bytes.iter().skip(1) {
            match *byte {
                b'p' => self.dash_p = true,
                b'S' => self.dash_s = true,
                _ => bail!(ParseError::InvalidUmaskOption { flag: word.clone() }),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
