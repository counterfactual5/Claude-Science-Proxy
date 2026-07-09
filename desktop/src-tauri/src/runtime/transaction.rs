/// 切换事务的提交/回滚决策（纯函数，spec §7）。live 路径难做确定性单测，故把决策抽出单独测。
#[derive(Debug, PartialEq)]
pub(crate) enum SwitchOutcome {
    Commit,           // scratch 校验过 + 正式代理探活健康 → 提交 active_id
    RollbackToOld,    // scratch 过但正式代理起/探活失败 → 杀候选、恢复旧代理、不提交
    AbortBeforeStart, // scratch 校验失败 → 根本没起正式代理、旧态零改动
}

/// 给定「候选 scratch 校验结果」与「正式代理探活结果」，决定切换事务走向。
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

/// 激活/切换是否跳过 scratch 上游校验（纯函数，修真机 P1）：只有用户显式 `skip_verify` 才跳；
/// 原生 adapter 不再豁免（旧行为 `native || skip_verify` 会让原生无效 key 提交为 active 并谎报「已切到」，
/// 首个真实推理才 401）。`native` 参数刻意保留：记录它曾是豁免条件、现已作废。
pub(crate) fn skip_scratch_verify(native: bool, skip_verify: bool) -> bool {
    let _ = native; // native 曾是豁免条件，现已作废（保留参数以固化回归防线）。
    skip_verify
}

/// 回滚结果 i18n 键（前端 `T()` 解析）。
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

    // ---------- P2-e: 回滚 i18n 键如实（恢复失败不得用 restored 键） ----------
    #[test]
    fn rollback_key_tells_truth_when_restore_failed() {
        assert_eq!(rollback_status_key(true), "rollbackRestored");
        assert_eq!(rollback_status_key(false), "rollbackProxyStopped");
    }

    // ---------- B3: 切换事务决策（纯函数，3 分支） ----------
    #[test]
    fn transaction_commits_only_when_healthy() {
        // scratch ok + real ok → 提交
        assert_eq!(decide_switch(true, true), SwitchOutcome::Commit);
        // scratch 校验失败 → 不起正式、不提交、旧态不动
        assert_eq!(decide_switch(false, false), SwitchOutcome::AbortBeforeStart);
        assert_eq!(decide_switch(false, true), SwitchOutcome::AbortBeforeStart);
        // scratch ok 但正式起/探活失败 → 杀候选、恢复旧、不提交
        assert_eq!(decide_switch(true, false), SwitchOutcome::RollbackToOld);
    }

    #[test]
    fn native_adapter_no_longer_bypasses_upstream_verify() {
        // 只有显式 skip_verify 才跳过；native 不再是豁免条件（旧行为的核心漏洞）。
        assert!(
            !skip_scratch_verify(true, false),
            "native 不得再豁免上游校验"
        );
        assert!(!skip_scratch_verify(false, false));
        assert!(skip_scratch_verify(false, true), "显式 skip_verify 才跳");
        assert!(skip_scratch_verify(true, true));
    }
}
