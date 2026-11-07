const WITHDRAWAL_STATES = new Set(['Active', 'Warning', 'Grace']);

export function canOwnerWithdraw(state, ownerMismatch = false) {
  return !ownerMismatch && WITHDRAWAL_STATES.has(state);
}

export function isMaturedPlanChange(status, state) {
  const pending = status?.pending_plan_change;
  if (!pending || !WITHDRAWAL_STATES.has(state)) return false;

  try {
    const nowNs = BigInt(status.current_timestamp_ns || '0');
    const effectiveAtNs = BigInt(pending.effective_at_ns || '0');
    const claimableAtNs = BigInt(status.last_heartbeat_timestamp_ns || '0') +
      (BigInt(status.heartbeat_interval_sec || '0') + BigInt(status.grace_period_sec || '0')) * 1_000_000_000n;
    return nowNs >= effectiveAtNs && effectiveAtNs < claimableAtNs;
  } catch {
    return false;
  }
}
