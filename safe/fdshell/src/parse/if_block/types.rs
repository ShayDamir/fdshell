use alloc::vec::Vec;
use sys::ScriptText;
use sys::ShortCStr;

pub struct ElifArm {
    pub cond: ScriptText,
    pub body: ScriptText,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
}

pub struct IfBlock {
    pub condition: ScriptText,
    pub then_body: ScriptText,
    pub elifs: Vec<ElifArm>,
    pub else_body: Option<ScriptText>,
    /// The condition's heredoc bodies (in operator order; empty when none).
    pub cond_bodies: Vec<ShortCStr>,
}
