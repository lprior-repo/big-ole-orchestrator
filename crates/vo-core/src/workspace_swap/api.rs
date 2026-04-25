use std::path::Path;

use crate::workspace_swap::swap::AtomicSwap;
use crate::workspace_swap::types::{RecoveryOutcome, SwapError, SwapStatus};

pub fn atomic_swap<P: AsRef<Path>>(workspace: P) -> Result<(), SwapError> {
    let swap = AtomicSwap::new(workspace);

    if let SwapStatus::Incomplete(phase) = swap.check_status()? {
        return Err(SwapError::RecoveryNeeded(phase));
    }

    swap.stage()?;
    swap.commit()?;

    Ok(())
}

pub fn recover_swap<P: AsRef<Path>>(workspace: P) -> Result<RecoveryOutcome, SwapError> {
    let swap = AtomicSwap::new(workspace);
    swap.recover()
}
