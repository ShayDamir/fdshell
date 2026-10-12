//! The `LoopControl` that reached the top level: a control signal that escaped
//! the loops and function calls that owned it.

use error_stack::{Report, bail};

use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;

/// Turn a `LoopControl` handed back by a whole script into an error.
///
/// The `break`/`continue` counts are clamped to `loop_depth` at the call site
/// (LESSONS "Clamp a `break`/`continue` count at the call site"), so a
/// `Break(_)`/`Continue(_)` reaching here means depth 0, which `run_simple`
/// already bails on, and a `Return` means no function call is running: all
/// three are unreachable defensive arms. `Exit` is errexit and needs no
/// action: the interactive REPL stops the current input, and the `-c` and
/// script-file callers fall through to the failing status in `last_status`.
pub(super) fn out_of_scope(control: LoopControl) -> Result<(), Report<CmdError>> {
    match control {
        LoopControl::Break(_) => bail!(CmdError::BreakOutsideLoop),
        LoopControl::Continue(_) => bail!(CmdError::ContinueOutsideLoop),
        LoopControl::Return => bail!(CmdError::ReturnOutsideFunction),
        LoopControl::Exit => Ok(()),
    }
}
