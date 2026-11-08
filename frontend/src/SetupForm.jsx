import React, { useMemo, useState } from 'react';
import { DEFAULT_KEEPER_ACCOUNT_ID, VAULT_ACCOUNT_ID } from './config.js';
import { estimateShareAmount, formatTokenAmount, shareBasisPoints } from './allocationEstimate.js';
import { setupVault } from './vaultClient.js';

const newDestination = (nearAccountId = '') => ({
  nearAccountId,
  ethereumAddress: '',
  bitcoinAddress: '',
  sharePercent: '',
});

export default function SetupForm({ account, wallet, depositAddresses, depositBalances, onComplete }) {
  const [destinations, setDestinations] = useState([newDestination(account)]);
  const [intervalDays, setIntervalDays] = useState('30');
  const [graceDays, setGraceDays] = useState('7');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const shareTotal = useMemo(
    () => destinations.reduce((sum, destination) => sum + (Number(destination.sharePercent) || 0), 0),
    [destinations],
  );

  const updateDestination = (index, field, value) => {
    setDestinations((current) => current.map((destination, i) => i === index ? { ...destination, [field]: value } : destination));
  };

  const submit = async (event) => {
    event.preventDefault();
    setError('');
    setNotice('');
    if (!VAULT_ACCOUNT_ID) {
      setError('The fresh shared Ember contract ID has not been configured in this frontend yet.');
      return;
    }
    if (!depositAddresses?.bitcoin?.address) {
      setError('Wait for the Bitcoin address to finish deriving, then try again.');
      return;
    }
    if (destinations.length < 1 || destinations.some((destination) => !destination.nearAccountId || !destination.ethereumAddress || !destination.bitcoinAddress || !destination.sharePercent)) {
      setError('Complete the NEAR account, Ethereum receiving address, Bitcoin Testnet receiving address, and share for every recipient.');
      return;
    }
    if (Math.round(shareTotal * 100) !== 10000) {
      setError('Destination shares must add up to exactly 100%.');
      return;
    }
    const intervalSeconds = Number(intervalDays) * 86400;
    const graceSeconds = Number(graceDays) * 86400;
    if (!Number.isFinite(intervalSeconds) || intervalSeconds < 1 || intervalSeconds > 31_536_000 ||
        !Number.isFinite(graceSeconds) || graceSeconds < 1 || graceSeconds > 31_536_000) {
      setError('Check-in interval and grace period must each be between 1 second and 365 days.');
      return;
    }

    setSaving(true);
    try {
      await setupVault(wallet, account, destinations, intervalSeconds, graceSeconds, DEFAULT_KEEPER_ACCOUNT_ID, depositAddresses);
      setNotice('The plan setup transaction completed. Refreshing your plan…');
      await onComplete();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'The wallet did not complete the setup transaction.');
    } finally {
      setSaving(false);
    }
  };

  return <section className="setup-panel" aria-labelledby="setup-title">
    <div className="setup-panel-heading">
      <div><span className="eyebrow-label">CREATE YOUR BACKUP PLAN</span><h2 id="setup-title">Set receiving addresses and check-in rhythm</h2></div>
      <span className="read-only-badge setup-network-badge">NEAR TESTNET</span>
    </div>
    <p className="setup-intro">Your wallet signs one setup transaction. The ETH and BTC addresses above are the vault addresses; the receiving addresses below receive their share if the plan reaches Ready for distribution. Estimates use current balances and are before network fees.</p>

    <form onSubmit={submit}>
      <div className="setup-destinations-heading"><div><h3>Receiving addresses</h3><p>Shares must total 100%. Bitcoin receiving addresses must be native Testnet SegWit addresses beginning with tb1q.</p></div><span className="setup-share-total">{shareTotal.toFixed(2)}% allocated</span></div>
      <div className="setup-destination-list">
        {destinations.map((destination, index) => <fieldset className="setup-destination-card" key={index}>
          <legend>Receiving address {String(index + 1).padStart(2, '0')}</legend>
          {destinations.length > 1 && <button className="remove-destination" type="button" onClick={() => setDestinations((current) => current.filter((_, i) => i !== index))} aria-label={`Remove receiving address ${index + 1}`}>Remove</button>}
          <label>NEAR account for manual fallback<input required autoComplete="off" value={destination.nearAccountId} onChange={(event) => updateDestination(index, 'nearAccountId', event.target.value.trim().toLowerCase())} placeholder="name.testnet" /></label>
          <div className="setup-address-fields">
            <label>Ethereum Sepolia receiving address<input required autoComplete="off" spellCheck="false" value={destination.ethereumAddress} onChange={(event) => updateDestination(index, 'ethereumAddress', event.target.value.trim())} placeholder="0x…" /></label>
            <label>Bitcoin Testnet receiving address<input required autoComplete="off" spellCheck="false" value={destination.bitcoinAddress} onChange={(event) => updateDestination(index, 'bitcoinAddress', event.target.value.trim())} placeholder="tb1q…" /></label>
          </div>
          <label className="setup-share-input">Share (%)<input required type="number" min="0.01" max="100" step="0.01" value={destination.sharePercent} onChange={(event) => updateDestination(index, 'sharePercent', event.target.value)} placeholder="100" /></label>
          <div className="setup-allocation-preview" aria-live="polite">
            <span className="setup-allocation-label">CURRENT SHARE ESTIMATE</span>
            {['Ethereum', 'Bitcoin'].map((chain) => {
              const balance = depositBalances?.[chain === 'Ethereum' ? 'ethereum' : 'bitcoin'];
              const amount = estimateShareAmount(balance?.balance, shareBasisPoints(destination.sharePercent));
              const label = amount == null
                ? !destination.sharePercent ? 'Enter a share' : balance?.loading ? 'Checking balance…' : balance?.error ? 'Balance unavailable' : 'Balance not loaded'
                : formatTokenAmount(amount, chain);
              return <div className="setup-allocation-row" key={chain}><span>{chain}</span><strong>{label}</strong></div>;
            })}
          </div>
        </fieldset>)}
      </div>
      <button className="add-destination-button" type="button" onClick={() => setDestinations((current) => [...current, newDestination()])}>+ Add another receiving address</button>

      <div className="setup-settings-grid">
        <label>Check-in interval <span>days</span><input required type="number" min="0.00002" max="365" step="0.01" value={intervalDays} onChange={(event) => setIntervalDays(event.target.value)} /><small>How often you plan to check in. Testnet supports short intervals for controlled demos.</small></label>
        <label>Grace period <span>days</span><input required type="number" min="0.00002" max="365" step="0.01" value={graceDays} onChange={(event) => setGraceDays(event.target.value)} /><small>Extra time after a missed check-in before the plan can run.</small></label>
      </div>

      <p className="setup-allocation-note">These are live estimates, not fixed payout amounts. Balances can change before distribution and network fees reduce the final transfers. Bitcoin distribution currently uses one confirmed UTXO per transaction.</p>

      <section className="setup-keeper-summary" aria-label="Automation configuration">
        <div className="setup-keeper-summary-heading"><strong>Automatic keeper</strong><span className="read-only-badge">{DEFAULT_KEEPER_ACCOUNT_ID ? 'CONFIGURED BY EMBER' : 'NOT CONFIGURED'}</span></div>
        <code>{DEFAULT_KEEPER_ACCOUNT_ID || 'Manual claims only'}</code>
        <small>{DEFAULT_KEEPER_ACCOUNT_ID
          ? 'Ember adds its shared keeper automatically. It can trigger claims when the plan is ready; it cannot check in or withdraw.'
          : 'No shared keeper is configured for this deployment, so claims must be started manually.'}</small>
      </section>
      {!VAULT_ACCOUNT_ID && <div className="setup-config-note" role="status">The frontend is waiting for the fresh shared contract account ID before setup can be submitted.</div>}
      {error && <div className="setup-error" role="alert">{error}</div>}
      {notice && <div className="setup-notice" role="status">{notice}</div>}
      <div className="setup-submit-row"><p>Setup is an on-chain NEAR Testnet transaction. Review all addresses and shares in your wallet before signing.</p><button className="button-primary" type="submit" disabled={saving || !VAULT_ACCOUNT_ID || !depositAddresses}>{saving ? 'Setting up…' : 'Review and set up plan'}<span aria-hidden="true">↗</span></button></div>
    </form>
  </section>;
}
