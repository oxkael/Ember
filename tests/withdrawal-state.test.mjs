import test from 'node:test';
import assert from 'node:assert/strict';
import { canOwnerWithdraw, isMaturedPlanChange } from '../frontend/src/withdrawalState.js';

function statusAt({ now = 20, effective = 10, heartbeat = 0, interval = 100, grace = 100 } = {}) {
  return {
    current_timestamp_ns: String(now * 1_000_000_000),
    last_heartbeat_timestamp_ns: String(heartbeat * 1_000_000_000),
    heartbeat_interval_sec: String(interval),
    grace_period_sec: String(grace),
    pending_plan_change: { effective_at_ns: String(effective * 1_000_000_000) },
  };
}

test('withdrawals are allowed only for the owner before Claimable', () => {
  for (const state of ['Active', 'Warning', 'Grace']) {
    assert.equal(canOwnerWithdraw(state), true, `${state} should allow owner withdrawals`);
  }
  assert.equal(canOwnerWithdraw('Claimable'), false);
  assert.equal(canOwnerWithdraw('Active', true), false, 'a non-owner must not get an enabled withdrawal action');
});

test('a matured plan update is usable before Claimable', () => {
  assert.equal(isMaturedPlanChange(statusAt(), 'Active'), true);
  assert.equal(isMaturedPlanChange(statusAt(), 'Grace'), true);
});

test('a plan update is not usable before its effective time', () => {
  assert.equal(isMaturedPlanChange(statusAt({ now: 9 }), 'Active'), false);
});

test('a plan update maturing at Claimable is not usable', () => {
  assert.equal(isMaturedPlanChange(statusAt({ effective: 200 }), 'Grace'), false);
});

test('a matured plan update is not usable after Claimable begins', () => {
  assert.equal(isMaturedPlanChange(statusAt({ now: 201 }), 'Claimable'), false);
});

test('missing or malformed pending timestamps fail closed', () => {
  assert.equal(isMaturedPlanChange({ pending_plan_change: null }, 'Active'), false);
  assert.equal(isMaturedPlanChange({ pending_plan_change: { effective_at_ns: 'bad' } }, 'Active'), false);
});
