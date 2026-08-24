/// Control-flow signals propagated up through the execution stack.
#[cfg_attr(test, derive(Debug))]
pub(crate) enum LoopControl {
    Break,
    Continue,
    /// Leave the current function (status already set by the `return` that raised it).
    Return,
}
