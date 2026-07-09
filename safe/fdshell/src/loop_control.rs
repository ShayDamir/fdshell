/// Loop control signals propagated through the execution stack.
#[cfg_attr(test, derive(Debug))]
pub(crate) enum LoopControl {
    Break,
    Continue,
}

/// Dispatch break/continue parsed lines to loop control signals.
pub(crate) fn dispatch_control(parsed: &crate::parse::ParsedLine) -> Option<LoopControl> {
    match parsed {
        crate::parse::ParsedLine::Break => Some(LoopControl::Break),
        crate::parse::ParsedLine::Continue => Some(LoopControl::Continue),
        _ => None,
    }
}
