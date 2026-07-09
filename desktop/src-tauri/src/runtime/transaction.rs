//! Profile switch commit/rollback decisions (pure functions, unit-tested).
//!
//! See also [`crate::runtime::profile_switch`] for the live transaction path.

/// Outcome of a switch transaction after scratch verify + formal proxy health.
#[derive(Debug, PartialEq)]
pub(crate) enum SwitchOutcome {
    Commit,           // scratch ok + formal proxy healthy → commit active_id
    RollbackToOld,    // scratch ok but formal start/health failed → kill candidate, restore old
    AbortBeforeStart, // scratch failed → formal proxy never started, state unchanged
}

/// Given scratch and formal-proxy health, decide how the switch transaction ends.
pub(crate) fn decide_switch(scratch_ok: bool, real_healthy: bool) -> SwitchOutcome {
    if !scratch_ok {
        return SwitchOutcome::AbortBeforeStart;
    }
    if real_healthy {
        SwitchOutcome::Commit
    } else {
        SwitchOutcome::RollbackToOld
    }
}

/// Whether to skip scratch upstream verify. Only explicit `skip_verify` skips; native adapters
/// are no longer exempt (that exemption allowed invalid native keys to become active).
/// `native` is retained in the signature only to lock in the regression guard.
pub(crate) fn skip_scratch_verify(native: bool, skip_verify: bool) -> bool {
    let _ = native;
    skip_verify
}

/// i18n key for rollback status hint (resolved in the frontend via `T()`).
pub(crate) fn rollback_status_key(restored: bool) -> &'static str {
    if restored {
        "rollbackRestored"
    } else {
        "rollbackProxyStopped"
    }
}

#[cfg(test)]
mod tests {
    use super::{decide_switch, rollback_status_key, skip_scratch_verify, SwitchOutcome};

    #[test]
    fn rollback_key_tells_truth_when_restore_failed() {
        assert_eq!(rollback_status_key(true), "rollbackRestored");
        assert_eq!(rollback_status_key(false), "rollbackProxyStopped");
    }

    #[test]
    fn decide_switch_three_branches() {
        assert_eq!(
            decide_switch(true, true),
            SwitchOutcome::Commit,
            "scratch ok + real ok → commit"
        );
        assert_eq!(
            decide_switch(false, true),
            SwitchOutcome::AbortBeforeStart,
            "scratch failed → abort"
        );
        assert_eq!(
            decide_switch(true, false),
            SwitchOutcome::RollbackToOld,
            "scratch ok but formal failed → rollback"
        );
    }

    #[test]
    fn skip_scratch_verify_only_when_explicit() {
        assert!(
            !skip_scratch_verify(true, false),
            "native must not skip upstream verify"
        );
        assert!(!skip_scratch_verify(false, false));
        assert!(skip_scratch_verify(true, true), "explicit skip_verify only");
        assert!(skip_scratch_verify(false, true));
    }
}
