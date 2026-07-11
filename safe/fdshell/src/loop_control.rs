/// Loop control signals propagated through the execution stack.
#[cfg_attr(test, derive(Debug))]
pub(crate) enum LoopControl {
    Break,
    Continue,
}
