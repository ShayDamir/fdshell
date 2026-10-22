use crate::redirect::RedirectDef;
use alloc::vec::Vec;
use sys::ScriptText;
use sys::ShortCStr;

pub struct ElifArm {
    pub cond: ScriptText,
    pub body: ScriptText,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
    /// The condition was terminated by the plain `&`: it runs as a background
    /// job (`condition_background`, task #356).
    pub cond_background: bool,
}

pub struct IfBlock {
    pub condition: ScriptText,
    pub then_body: ScriptText,
    pub elifs: Vec<ElifArm>,
    pub else_body: Option<ScriptText>,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
    /// The condition was terminated by the plain `&`: it runs as a background
    /// job (`condition_background`, task #356).
    pub cond_background: bool,
    /// The redirections written after the closing `fi` (task #298,
    /// `parse::block_tail`); they apply to the whole `if`.
    pub redirects: Vec<RedirectDef>,
}
