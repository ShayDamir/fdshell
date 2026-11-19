// mutants: -C --lib -C --test -C read -C --test -C trap -- -E 'test(/^intercept::read::tests::/) | binary(read) | binary(trap)'

use error_stack::{Report, ResultExt};

use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, ScriptText, ShortCStr, Trace};

use super::words::split_fields;

/// Write the line's values into the variable store: bash's REPLY extension for
/// an empty target list, the field split for named ones. The no-name branch
/// takes the raw line — no trim, no splitting, both are named-target rules
/// (task #604) — and it is this call that guarantees `split_fields` its
/// `num_targets >= 1`; the empty-line bail (EOF/timeout, per kind) is the
/// caller's.
pub(crate) fn assign_line(
    targets: &[ShortCStr],
    data: &[u8],
    text: &ScriptText,
    origin: &Origin,
    cell: &ForkCell<ShellState>,
) -> Result<(), Report<CmdError>> {
    let mut state = cell.borrow_mut().change_context(CmdError::Read)?;
    if targets.is_empty() {
        let value = ShortCStr::from_vec(data.to_vec()).change_context(CmdError::Read)?;
        state.set_var(
            c"REPLY".into(),
            ImportedStr::new(value, Trace::at(text.start, origin.clone())),
        );
    } else {
        let fields = split_fields(data, targets.len());
        for (i, name) in targets.iter().enumerate() {
            let field = fields.get(i).map(|v| v.as_slice()).unwrap_or(&[]);
            let var_name = name.strip_prefix(b"$").unwrap_or_else(|| name.clone());
            let s = ShortCStr::from_vec(field.to_vec()).change_context(CmdError::Read)?;
            state.set_var(
                var_name,
                ImportedStr::new(s, Trace::at(text.start, origin.clone())),
            );
        }
    }

    Ok(())
}
