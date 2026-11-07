import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { NEAR_NETWORK, VAULT_ACCOUNT_ID } from './config.js';
import { bitcoinTestnetWitnessProgramToAddress } from './chainSignatures.js';
import SetupForm from './SetupForm.jsx';
import { readBitcoinBalance, readEthereumBalance } from './balanceClient.js';
import { checkReportedTransaction } from './confirmationClient.js';
import { cancelPendingPlanChange, deriveVaultDepositAddresses, pingVault, proposeWithdrawalDestinations, readVaultStatus } from './vaultClient.js';
import { broadcastBtcWithdrawal, broadcastEthWithdrawal, getBtcWithdrawalPreview, getEthWithdrawalPreview, requestBtcWithdrawal, requestEthWithdrawal, storeEthWithdrawalSession } from './withdrawalClient.js';
import { canOwnerWithdraw, isMaturedPlanChange } from './withdrawalState.js';
import './styles.css';

let connectorPromise;
function getConnector() {
  if (!connectorPromise) {
    connectorPromise = import('@hot-labs/near-connect').then(({ NearConnector }) => new NearConnector({ network: NEAR_NETWORK, features: { testnet: true }, autoConnect: false }));
  }
  return connectorPromise;
}

function Mark({ small = false }) {
  return <span className={`ember-mark ${small ? 'ember-mark-small' : ''}`} aria-hidden="true"><i /><b /></span>;
}

function Arrow({ diagonal = false }) {
  return <svg viewBox="0 0 20 20" aria-hidden="true"><path d={diagonal ? 'M5 15 15 5M6 5h9v9' : 'M3 10h13m-5-5 5 5-5 5'} /></svg>;
}

function ChainIllustration() {
  return <div className="chain-illustration" aria-label="NEAR coordinates threshold signing for Bitcoin and Ethereum">
    <div className="illustration-glow" />
    <div className="diagram-orbit orbit-a" /><div className="diagram-orbit orbit-b" /><div className="diagram-orbit orbit-c" />
    <div className="diagram-lines"><span /><span /></div>
    <div className="diagram-core"><Mark /><small>EMBER PLAN</small></div>
    <div className="diagram-node node-near"><span className="near-glyph">N</span><small>NEAR</small></div>
    <div className="diagram-node node-bitcoin"><span className="bitcoin-glyph">₿</span><small>BITCOIN</small></div>
    <div className="diagram-node node-ethereum"><span className="ethereum-glyph">◆</span><small>ETHEREUM</small></div>
    <div className="diagram-label label-mpc">THRESHOLD SIGNING</div>
    <div className="diagram-label label-native">NATIVE ASSETS</div>
    <div className="orbit-spark spark-one" /><div className="orbit-spark spark-two" /><div className="orbit-spark spark-three" />
  </div>;
}

function Landing({ onConnect, connecting, restoringWallet, error }) {
  return <div className="landing-page">
    <header className="landing-nav">
      <a className="wordmark" href="#top"><Mark /><span>ember<small>PROTOCOL</small></span></a>
      <nav className="landing-links" aria-label="Page navigation"><a href="#how-it-works">How it works</a><a href="#why-near">Why NEAR</a><a href="#faq">FAQs</a></nav>
      <button className="nav-connect" onClick={onConnect} disabled={connecting || restoringWallet}>{restoringWallet ? 'Checking wallet…' : connecting ? 'Connecting…' : 'Connect wallet'} <Arrow /></button>
    </header>

    <main id="top">
      <section className="hero-section">
        <div className="hero-copy">
          <div className="hero-eyebrow"><span /> A BACKUP PLAN FOR YOUR DIGITAL ASSETS</div>
          <h1>Your assets deserve<br />a <em>plan B.</em></h1>
          <p className="hero-description">Ember helps you prepare for lost access, emergencies, or time away—with a check-in plan and destinations across Bitcoin and Ethereum.</p>
          <div className="hero-actions"><button className="button-primary" onClick={onConnect} disabled={connecting || restoringWallet}>{restoringWallet ? 'Checking wallet session…' : connecting ? 'Connecting to NEAR…' : 'Connect NEAR wallet'} <Arrow /></button><a className="text-link" href="#how-it-works">Explore how it works <span><Arrow diagonal /></span></a></div>
          <p className="hero-footnote">NEAR Testnet preview <span>·</span> Wallet connection loads the current plan</p>
          {error && <div className="connect-error" role="alert">{error}</div>}
        </div>
        <div className="hero-visual"><ChainIllustration /><div className="visual-caption"><span className="caption-mark"><Mark small /></span><span><strong>One plan, multiple chains</strong><small>Coordinated with NEAR Chain Signatures</small></span><span className="caption-arrow"><Arrow diagonal /></span></div></div>
        <div className="hero-side-note"><span>01</span><i /> PREPARE FOR THE UNEXPECTED</div>
      </section>

      <section className="trust-bar" aria-label="Supported technology">
        <div className="trust-intro">A considered way to<br />back up digital assets.</div>
        <div className="trust-item"><span className="trust-icon native-icon">₿</span><span><strong>Native Bitcoin</strong><small>Testnet destinations</small></span></div>
        <div className="trust-item"><span className="trust-icon eth-icon">◆</span><span><strong>Native Ethereum</strong><small>Sepolia destinations</small></span></div>
        <div className="trust-item near-trust"><span className="trust-icon near-trust-icon">N</span><span><strong>NEAR Chain Signatures</strong><small>Threshold MPC coordination</small></span></div>
      </section>

      <section className="how-section" id="how-it-works">
        <div className="section-heading"><div><span className="eyebrow-label">A SIMPLE ROUTINE</span><h2>Make a plan.<br /><em>Then get on with life.</em></h2></div><p>Ember gives your digital assets a backup route that follows your instructions—not a schedule you have to think about every day.</p></div>
        <div className="steps-grid">
          <article className="step-card"><span className="step-number">01</span><div className="step-symbol destination-symbol"><svg viewBox="0 0 24 24"><circle cx="9" cy="8" r="3"/><path d="M3 20a6 6 0 0 1 12 0m2-9h5m-2.5-2.5v5" /></svg></div><h3>Choose destinations</h3><p>Set trusted destinations or a personal backup wallet, with shares that add up to your plan.</p><span className="step-foot">YOUR PLAN, YOUR CHOICE</span></article>
          <article className="step-card"><span className="step-number">02</span><div className="step-symbol checkin-symbol"><svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2" /></svg></div><h3>Check in from time to time</h3><p>Your check-in keeps the plan active. You choose a rhythm that fits your life.</p><span className="step-foot">A SMALL, REGULAR SIGNAL</span></article>
          <article className="step-card"><span className="step-number">03</span><div className="step-symbol route-symbol"><svg viewBox="0 0 24 24"><path d="M5 12h6m0 0-3-3m3 3-3 3m4-8 4-3m-4 13 4 3m0-16h4v4m-4 12h4v-4" /></svg></div><h3>Keep a backup route</h3><p>If check-ins stop and the grace period passes, configured automation can route assets to your destinations.</p><span className="step-foot">AUTOMATION WHEN CONFIGURED</span></article>
        </div>
      </section>

      <section className="destinations-section">
        <div className="destinations-copy"><span className="eyebrow-label">BUILT AROUND REAL LIFE</span><h2>Backup means<br /><em>different things to different people.</em></h2><p>Some people plan for someone they trust. Others want a second place for their own assets. Ember is designed to support both.</p><a className="inline-arrow-link" href="#faq">Read common questions <Arrow /></a></div>
        <div className="destination-showcase">
          <article className="destination-tile people-tile"><div className="tile-top"><span className="tile-icon people-tile-icon"><svg viewBox="0 0 24 24"><circle cx="9" cy="8" r="3"/><path d="M3 20a6 6 0 0 1 12 0m3-11h4m-2-2v4" /></svg></span><span className="tile-number">OPTION 01</span></div><h3>People you trust</h3><p>Share your plan across the people and wallets you choose.</p><div className="tile-lines"><i /><i /><i /></div></article>
          <article className="destination-tile self-tile"><div className="tile-top"><span className="tile-icon self-tile-icon"><svg viewBox="0 0 24 24"><path d="M12 22s8-4 8-11V5l-8-3-8 3v6c0 7 8 11 8 11Z"/><path d="m9 12 2 2 4-4" /></svg></span><span className="tile-number">OPTION 02</span></div><h3>Your personal backup</h3><p>Keep a secondary wallet ready for your own recovery plan.</p><div className="wallet-chip"><span className="wallet-chip-dot" /> YOUR SECONDARY WALLET</div></article>
          <span className="showcase-stamp"><Mark small /><span>DESIGNED FOR<br />THE UNEXPECTED</span></span>
        </div>
      </section>

      <section className="near-section" id="why-near">
        <div className="near-section-mark"><Mark /></div><div className="near-section-copy"><span className="eyebrow-label">THE TECHNOLOGY UNDER THE HOOD</span><h2>One NEAR contract.<br /><em>Native assets on other chains.</em></h2><p>NEAR Chain Signatures lets Ember coordinate threshold signing for transactions on external networks. That means a single backup plan can be designed for native Bitcoin and Ethereum assets—without routing through wrapped tokens.</p><div className="near-proof-row"><span><i className="proof-dot" /> NEAR smart contract</span><span><i className="proof-dot" /> Threshold MPC</span><span><i className="proof-dot" /> Bitcoin + Ethereum</span></div></div><div className="near-side-label">POWERED BY NEAR</div>
      </section>

      <section className="faq-section" id="faq">
        <div className="faq-heading"><span className="eyebrow-label">GOOD TO KNOW</span><h2>Questions, answered.</h2><p>A clear plan starts with knowing how it works.</p></div>
        <div className="faq-list">
          <details><summary>What is Ember Protocol?<span className="faq-plus" /></summary><p>Ember is an emergency backup plan for digital assets. It uses periodic check-ins and a grace period to determine when a configured distribution plan may activate.</p></details>
          <details><summary>Who is this for?<span className="faq-plus" /></summary><p>Anyone who wants a backup route for their digital assets—whether that means planning for a loss of access, an emergency, a long absence, or a personal recovery wallet.</p></details>
          <details><summary>Do my chosen destinations need to do anything?<span className="faq-plus" /></summary><p>The intended primary path is automatic when keeper automation is configured and running. A manual fallback may require a NEAR account and wallet. The interface will show the requirements for the active contract.</p></details>
          <details><summary>What does NEAR do in the system?<span className="faq-plus" /></summary><p>The NEAR contract coordinates the plan and requests threshold signatures through NEAR Chain Signatures for transactions on supported external networks.</p></details>
          <details><summary>Is this connected to mainnet?<span className="faq-plus" /></summary><p>No. This frontend currently connects to NEAR Testnet and reads the project&apos;s testnet contract. It does not send transactions from this preview.</p></details>
        </div>
      </section>

      <section className="closing-cta"><div className="closing-orbit" /><span className="eyebrow-label">A LITTLE PREPARATION GOES A LONG WAY</span><h2>Start with a plan<br /><em>that feels like yours.</em></h2><button className="button-light" onClick={onConnect} disabled={connecting}>{connecting ? 'Connecting…' : 'Connect NEAR wallet'} <Arrow /></button><small>NEAR Testnet · Read-only dashboard preview</small></section>
    </main>

    <footer className="landing-footer"><a className="wordmark footer-brand" href="#top"><Mark small /><span>ember<small>PROTOCOL</small></span></a><p>Emergency backup for multi-chain assets.</p><span className="footer-powered">POWERED BY NEAR CHAIN SIGNATURES</span><span className="footer-copyright">TESTNET PREVIEW</span></footer>
  </div>;
}

function shortAccount(value) {
  return value && value.length > 25 ? `${value.slice(0, 10)}…${value.slice(-8)}` : value;
}

function formatTime(value) {
  const seconds = Math.max(0, Number(value) || 0);
  if (seconds === 0) return 'Ready for distribution';
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  if (days) return `${days}d ${hours}h remaining`;
  const minutes = Math.floor(seconds / 60);
  return `${hours}h ${minutes % 60}m remaining`;
}

function displayState(state) {
  const raw = typeof state === 'string' ? state : Object.keys(state || {})[0] || 'Unknown';
  const labels = { Warning: 'Reminder', Grace: 'Grace period', Claimable: 'Ready for distribution', Uninitialized: 'Not set up' };
  return { raw, label: labels[raw] || raw };
}

function enumValue(value) {
  return typeof value === 'string' ? value : Object.keys(value || {})[0] || 'Unknown';
}

function deliveryLabel(value) {
  const raw = enumValue(value);
  return raw === 'Done' ? 'Delivered (reported)' : raw;
}

function formatTimestamp(timestampNs) {
  const timestamp = Number(timestampNs || 0);
  if (!timestamp) return '—';
  return new Date(timestamp / 1_000_000).toLocaleString();
}

function formatWithdrawalAmount(record) {
  const chain = enumValue(record.chain);
  const value = BigInt(record.amount || 0);
  const decimals = chain === 'Bitcoin' ? 8 : 18;
  const divisor = 10n ** BigInt(decimals);
  const whole = value / divisor;
  const visibleDecimals = chain === 'Bitcoin' ? 8 : 6;
  const fraction = (value % divisor).toString().padStart(decimals, '0').slice(0, visibleDecimals).replace(/0+$/, '');
  const unit = chain === 'Bitcoin' ? 'BTC' : 'ETH';
  return `${whole}${fraction ? `.${fraction}` : ''} ${unit}`;
}

function withdrawalDestination(record) {
  if (enumValue(record.chain) !== 'Bitcoin') return record.destination || 'Destination unavailable';
  try {
    return bitcoinTestnetWitnessProgramToAddress(record.destination);
  } catch {
    return 'Bitcoin Testnet destination';
  }
}

function safeBitcoinDestination(value) {
  if (!value) return '';
  try { return bitcoinTestnetWitnessProgramToAddress(value); }
  catch { return ''; }
}

function formatDepositBalance(value, symbol, decimals) {
  if (value == null) return null;
  const base = BigInt(value);
  const divisor = 10n ** BigInt(decimals);
  const fraction = (base % divisor).toString().padStart(decimals, '0').slice(0, decimals === 8 ? 8 : 6).replace(/0+$/, '');
  return `${base / divisor}${fraction ? `.${fraction}` : ''} ${symbol}`;
}

function DepositBalance({ chain, result }) {
  const symbol = chain === 'ethereum' ? 'ETH' : 'BTC';
  const decimals = chain === 'ethereum' ? 18 : 8;
  const balance = formatDepositBalance(result?.balance, symbol, decimals);
  const checking = result?.loading;
  return <div className="deposit-balance" aria-live="polite">
    <div className="deposit-balance-value"><span>Current balance</span><strong>{balance || (checking ? 'Checking…' : result?.error ? 'Unavailable' : 'Not checked yet')}</strong></div>
    {result?.asOf && <small>As of {new Date(result.asOf).toLocaleTimeString()}</small>}
    {result?.error && <small className="deposit-balance-error">{balance ? 'Refresh failed; showing the last result.' : 'Could not load this balance. Try again.'}</small>}
  </div>;
}

function DepositAddresses({ addresses, balances, refreshing, onRefresh, addressError }) {
  return <section className="deposit-addresses" aria-labelledby="deposit-addresses-title">
    <div className="deposit-addresses-heading">
      <div><span className="eyebrow-label">NEAR CHAIN SIGNATURES</span><h2 id="deposit-addresses-title">Your testnet deposit addresses</h2></div>
      <div className="deposit-addresses-actions"><span className="read-only-badge">POWERED BY NEAR MPC</span><button className="refresh-balances-button" type="button" onClick={onRefresh} disabled={!addresses || refreshing}>{refreshing ? 'Checking balances…' : 'Refresh balances'}</button></div>
    </div>
    {addresses ? <div className="deposit-address-grid">
      <article className="deposit-address-item"><span>ETHEREUM · SEPOLIA</span><code>{addresses.ethereum.address}</code><DepositBalance chain="ethereum" result={balances.ethereum} /></article>
      <article className="deposit-address-item"><span>BITCOIN · TESTNET</span><code>{addresses.bitcoin.address}</code><DepositBalance chain="bitcoin" result={balances.bitcoin} /></article>
    </div> : <p className={addressError ? 'deposit-address-error' : 'deposit-address-pending'} role={addressError ? 'alert' : undefined}>{addressError || 'Deriving addresses…'}</p>}
    <p className="deposit-balance-footnote">Balances come from public testnet services and may update with a delay.</p>
  </section>;
}

function ReportedConfirmation({ chain, txHash }) {
  const [sources, setSources] = useState([]);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState('');
  const refresh = useCallback(async () => {
    setChecking(true); setError('');
    try { setSources(await checkReportedTransaction(chain, txHash)); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not check the reported transaction.'); }
    finally { setChecking(false); }
  }, [chain, txHash]);
  useEffect(() => { void refresh(); }, [refresh]);
  return <div className="reported-confirmation" aria-live="polite">
    <div className="reported-confirmation-heading"><span>Public source checks</span><button type="button" onClick={refresh} disabled={checking}>{checking ? 'Checking…' : 'Refresh'}</button></div>
    {error && <p className="reported-confirmation-error">{error}</p>}
    {sources.map((source) => <div className="reported-source-row" key={source.source}><span><strong>{source.source}</strong><small>{source.state}{source.confirmations ? ` · ${source.confirmations} confirmations` : ''}{source.blockNumber ? ` · block ${source.blockNumber}` : ''}</small></span><small>{source.asOf ? `As of ${new Date(source.asOf).toLocaleTimeString()}` : ''}{source.error ? ` · ${source.error}` : ''}</small></div>)}
    <p className="reported-confirmation-note">The transaction hash was reported by a wallet or keeper. These independent lookups are informational and are not checked by the NEAR contract.</p>
  </div>;
}

function DestinationList({ destinations }) {
  return <section className="dashboard-destinations">{destinations.length ? destinations.map((slot, index) => {
    let bitcoinAddress = 'Bitcoin Testnet address not set';
    try {
      if (slot.heir?.bitcoin_witness_program) {
        bitcoinAddress = bitcoinTestnetWitnessProgramToAddress(slot.heir.bitcoin_witness_program);
      }
    } catch {
      bitcoinAddress = 'Saved Bitcoin destination could not be decoded';
    }
    return <article className="dashboard-destination" key={`${slot.heir?.near_account_id || index}`}>
      <div className="destination-avatar">{String(index + 1).padStart(2, '0')}</div>
      <div className="dashboard-destination-main">
        <div className="dashboard-destination-heading"><div className="dashboard-destination-info"><span className="eyebrow-label">DESTINATION {String(index + 1).padStart(2, '0')}</span><strong>{slot.heir?.near_account_id || 'NEAR account not set'}</strong></div><span className="share-pill">{(Number(slot.heir?.share_basis_points) || 0) / 100}% share</span><span className={`slot-status slot-${enumValue(slot.status).toLowerCase()}`}>{deliveryLabel(slot.status)}</span></div>
        <div className="destination-chain-addresses"><div><span>ETHEREUM · SEPOLIA</span><code>{slot.heir?.ethereum_address || 'Ethereum address not set'}</code></div><div><span>BITCOIN · TESTNET</span><code>{bitcoinAddress}</code></div></div>
      </div>
    </article>;
  }) : <div className="dashboard-empty"><h3>No destinations are configured</h3><p>This contract does not have a backup plan set up yet.</p></div>}</section>;
}

function WithdrawalPanel({ account, wallet, status, state, ownerMismatch, addresses, vaultId, onReload }) {
  const activeDestinations = {
    ethereum: status.owner_eth_address || '',
    bitcoin: safeBitcoinDestination(status.owner_btc_witness_program),
  };
  const pending = status.pending_plan_change;
  const pendingDestinations = {
    ethereum: pending?.owner_eth_address || '',
    bitcoin: safeBitcoinDestination(pending?.owner_btc_witness_program),
  };
  const maturedPlanChange = isMaturedPlanChange(status, state.raw);
  // The contract applies a matured update inside withdraw_eth/withdraw_btc.
  // Until that transaction runs, status() still reports the old active addresses.
  const withdrawalDestinations = maturedPlanChange ? pendingDestinations : activeDestinations;
  const [ethInput, setEthInput] = useState(pendingDestinations.ethereum || activeDestinations.ethereum);
  const [btcInput, setBtcInput] = useState(pendingDestinations.bitcoin || activeDestinations.bitcoin);
  const [busy, setBusy] = useState('');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [ethPreview, setEthPreview] = useState(null);
  const [btcPreview, setBtcPreview] = useState(null);
  const canWithdraw = canOwnerWithdraw(state.raw, ownerMismatch);
  const ethRecord = [...(status.withdrawal_records || [])].reverse().find((record) => enumValue(record.chain) === 'Ethereum' && ['Pending', 'Signed'].includes(enumValue(record.status)));
  const btcRecord = [...(status.withdrawal_records || [])].reverse().find((record) => enumValue(record.chain) === 'Bitcoin' && ['Pending', 'Signed'].includes(enumValue(record.status)));

  async function waitForSignature(chain, startedAtNs, nonce) {
    for (let i = 0; i < 20; i += 1) {
      const next = await onReload();
      const record = [...(next?.withdrawal_records || [])].reverse().find((item) => enumValue(item.chain) === chain && enumValue(item.status) === 'Signed' &&
        BigInt(item.timestamp_ns) >= BigInt(startedAtNs) && (nonce == null || String(item.nonce) === String(nonce)));
      if (record) return record;
      await new Promise((resolve) => setTimeout(resolve, 3000));
    }
    throw new Error('The request was submitted, but the contract has not recorded an MPC signature yet. Refresh later; do not submit another withdrawal.');
  }

  async function registerDestinations(event) {
    event.preventDefault();
    setBusy('destinations'); setError(''); setNotice('');
    try {
      await proposeWithdrawalDestinations(wallet, account, ethInput, btcInput, vaultId);
      await onReload();
      setNotice('Destination change submitted. The active addresses remain in effect for 14 days.');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not submit the destination change.'); }
    finally { setBusy(''); }
  }

  async function cancelChange() {
    setBusy('cancel'); setError(''); setNotice('');
    try { await cancelPendingPlanChange(wallet, account, vaultId); await onReload(); setNotice('Pending plan change cancelled.'); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not cancel the pending change.'); }
    finally { setBusy(''); }
  }

  async function previewEth() {
    setBusy('preview-eth'); setError(''); setNotice('');
    try { setEthPreview(await getEthWithdrawalPreview(addresses.ethereum.address, status)); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not calculate the Ethereum withdrawal.'); }
    finally { setBusy(''); }
  }

  async function previewBtc() {
    setBusy('preview-btc'); setError(''); setNotice('');
    try { setBtcPreview(await getBtcWithdrawalPreview(addresses.bitcoin.address)); }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not calculate the Bitcoin withdrawal.'); }
    finally { setBusy(''); }
  }

  async function startEth() {
    setBusy('eth'); setError(''); setNotice('');
    try {
      if (!ethPreview) throw new Error('Calculate the withdrawal amount before requesting a signature.');
      const startedAtNs = status.current_timestamp_ns || '0';
      storeEthWithdrawalSession(account, { nonce: ethPreview.nonce }, ethPreview.gasPrice);
      const preview = await requestEthWithdrawal(wallet, account, status, addresses.ethereum.address, vaultId, ethPreview);
      const record = await waitForSignature('Ethereum', startedAtNs, preview.nonce);
      storeEthWithdrawalSession(account, record, preview.gasPrice);
      setEthPreview(null);
      await onReload();
      setNotice('Ethereum withdrawal signed. Review the recorded transaction, then broadcast it when ready.');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not request an Ethereum withdrawal.'); }
    finally { setBusy(''); }
  }

  async function startBtc() {
    setBusy('btc'); setError(''); setNotice('');
    try {
      if (!btcPreview) throw new Error('Calculate the withdrawal amount before requesting a signature.');
      const startedAtNs = status.current_timestamp_ns || '0';
      await requestBtcWithdrawal(wallet, account, status, addresses.bitcoin.address, vaultId, btcPreview);
      await waitForSignature('Bitcoin', startedAtNs, null);
      setBtcPreview(null);
      await onReload();
      setNotice('Bitcoin withdrawal signed. Review the recorded transaction, then broadcast it when ready.');
    } catch (cause) { setError(cause instanceof Error ? cause.message : 'Could not request a Bitcoin withdrawal.'); }
    finally { setBusy(''); }
  }

  async function broadcast(record) {
    const chain = record.chain;
    setBusy(`broadcast-${chain}`); setError(''); setNotice('');
    try {
      const txid = chain === 'Ethereum'
        ? await broadcastEthWithdrawal(wallet, account, record, status, addresses, vaultId)
        : await broadcastBtcWithdrawal(wallet, account, record, status, addresses, vaultId);
      await onReload();
      setNotice(`${chain} transaction broadcast and reported to the contract: ${txid}`);
    } catch (cause) { setError(cause instanceof Error ? cause.message : `Could not broadcast the ${chain} withdrawal.`); }
    finally { setBusy(''); }
  }

  const effectiveAt = pending?.effective_at_ns ? formatTimestamp(pending.effective_at_ns) : null;
  const ethReady = enumValue(ethRecord?.status) === 'Signed';
  const btcReady = enumValue(btcRecord?.status) === 'Signed';
  const ethActionOutstanding = status.eth_outstanding_nonce != null;
  const btcClaimOutstanding = ['Pending', 'Signed'].includes(enumValue(status.btc_slot?.status));

  return <section className="withdrawal-view" aria-labelledby="withdrawal-title">
    <div className="withdrawal-intro"><span className="eyebrow-label">OWNER CONTROLS</span><h2 id="withdrawal-title">Withdraw to your registered wallets</h2><p>Withdrawals use the active registered destination. They do not reset your check-in timer. Destination updates take 14 days to become active.</p></div>
    {ownerMismatch && <div className="withdrawal-notice" role="status">Only the account that owns this plan can manage withdrawals.</div>}
    {!canWithdraw && !ownerMismatch && <div className="withdrawal-notice" role="status">Withdrawals are unavailable once the plan reaches Ready for distribution. This plan is currently {state.label}.</div>}
    {canWithdraw && ethActionOutstanding && !ethRecord && <div className="withdrawal-notice" role="status">An Ethereum claim or withdrawal is already outstanding. Resolve it before starting another Ethereum action.</div>}
    {canWithdraw && btcClaimOutstanding && !btcRecord && <div className="withdrawal-notice" role="status">A Bitcoin claim is already outstanding. Resolve it before starting a Bitcoin withdrawal.</div>}
    {error && <div className="setup-error" role="alert">{error}</div>}
    {notice && <div className="setup-notice" role="status">{notice}</div>}
    {pending && <div className="pending-plan-card withdrawal-pending"><span className="eyebrow-label">PENDING PLAN CHANGE</span><h3>Update scheduled</h3><p>{maturedPlanChange ? 'The 14-day wait is complete. A withdrawal request will apply this update and use the updated destination.' : `Its effective time is ${effectiveAt}. Active destinations remain in use until then.`} Cancelling here cancels the entire pending plan update, including any recipient or keeper edits.</p><button type="button" className="withdrawal-secondary-button" onClick={cancelChange} disabled={busy !== '' || ownerMismatch}>{busy === 'cancel' ? 'Cancelling…' : 'Cancel pending update'}</button></div>}
    <div className="withdrawal-destination-card"><div className="withdrawal-card-heading"><div><span className="eyebrow-label">REGISTERED DESTINATIONS</span><h3>Where withdrawals can go</h3></div>{activeDestinations.ethereum && activeDestinations.bitcoin && <span className="read-only-badge">ACTIVE</span>}</div>
        {activeDestinations.ethereum || activeDestinations.bitcoin ? <div className="withdrawal-address-list">
        <div><span>ETHEREUM · SEPOLIA</span><code>{activeDestinations.ethereum || 'Not registered'}</code></div>
        <div><span>BITCOIN · TESTNET</span><code>{activeDestinations.bitcoin || 'Not registered'}</code></div>
      </div> : <p className="withdrawal-explain">No withdrawal destinations are active yet. Register both addresses below; the contract applies them after 14 days.</p>}
      {canWithdraw && <form className="withdrawal-address-form" onSubmit={registerDestinations}><label>Ethereum Sepolia address<input value={ethInput} onChange={(event) => setEthInput(event.target.value)} placeholder="0x…" autoComplete="off" /></label><label>Bitcoin Testnet address<input value={btcInput} onChange={(event) => setBtcInput(event.target.value)} placeholder="tb1q…" autoComplete="off" /></label><button type="submit" className="withdrawal-secondary-button" disabled={busy !== ''}>{busy === 'destinations' ? 'Submitting…' : pending ? 'Update pending destinations' : 'Register or update destinations'}</button></form>}
    </div>
    {maturedPlanChange && <div className="withdrawal-notice" role="status"><strong>Destination update ready.</strong> The next withdrawal request applies the pending plan update and uses these addresses:
      <div className="withdrawal-address-list"><div><span>ETHEREUM · SEPOLIA</span><code>{pendingDestinations.ethereum || 'Not registered'}</code></div><div><span>BITCOIN · TESTNET</span><code>{pendingDestinations.bitcoin || 'Not registered'}</code></div></div>
    </div>}
    <div className="withdrawal-chain-grid">
      <article className="withdrawal-chain-card"><span className="eyebrow-label">ETHEREUM · SEPOLIA</span><h3>Withdraw ETH</h3><p>Uses the full current balance minus the estimated network fee. The contract enforces its fee caps.</p>{!withdrawalDestinations.ethereum && <p className="withdrawal-small-note">An active Ethereum destination is required.</p>}{ethRecord && <div className="withdrawal-outstanding"><strong>Outstanding transaction · {ethRecord.status}</strong><code>{ethRecord.amount ? formatWithdrawalAmount(ethRecord) : 'Waiting for signature'}</code></div>}
        {!ethRecord && !ethPreview && <button type="button" className="withdrawal-action-button" onClick={previewEth} disabled={busy !== '' || !canWithdraw || ethActionOutstanding || !withdrawalDestinations.ethereum || !addresses?.ethereum?.address}>{busy === 'preview-eth' ? 'Calculating…' : 'Withdraw ETH'}</button>}
        {ethPreview && !ethRecord && <div className="withdrawal-preview"><p>Estimated fee <strong>{formatDepositBalance(ethPreview.estimatedFee, 'ETH', 18)}</strong></p><p>Amount to receive <strong>{formatDepositBalance(ethPreview.amount, 'ETH', 18)}</strong></p><button type="button" className="button-primary" onClick={startEth} disabled={busy !== ''}>{busy === 'eth' ? 'Requesting signature…' : 'Confirm and request signature'}</button><button type="button" className="withdrawal-text-button" onClick={() => setEthPreview(null)} disabled={busy !== ''}>Cancel preview</button></div>}
        {canWithdraw && ethReady && <button type="button" className="button-primary" onClick={() => broadcast(ethRecord)} disabled={busy !== ''}>{busy === 'broadcast-Ethereum' ? 'Broadcasting…' : 'Broadcast signed ETH withdrawal'}</button>}
      </article>
      <article className="withdrawal-chain-card"><span className="eyebrow-label">BITCOIN · TESTNET</span><h3>Withdraw BTC</h3><p>Sweeps one confirmed UTXO to your active registered Bitcoin address. The contract enforces its fee caps.</p>{!withdrawalDestinations.bitcoin && <p className="withdrawal-small-note">An active Bitcoin destination is required.</p>}{btcRecord && <div className="withdrawal-outstanding"><strong>Outstanding transaction · {btcRecord.status}</strong><code>{btcRecord.amount ? formatWithdrawalAmount(btcRecord) : 'Waiting for signature'}</code></div>}
        {!btcRecord && !btcPreview && <button type="button" className="withdrawal-action-button" onClick={previewBtc} disabled={busy !== '' || !canWithdraw || btcClaimOutstanding || !withdrawalDestinations.bitcoin || !addresses?.bitcoin?.address}>{busy === 'preview-btc' ? 'Calculating…' : 'Withdraw BTC'}</button>}
        {btcPreview && !btcRecord && <div className="withdrawal-preview"><p>Selected UTXO <strong>{btcPreview.valueSats.toLocaleString()} sats</strong></p><p>Estimated fee <strong>{btcPreview.estimatedFee} sats at {btcPreview.feeRate} sat/vB</strong></p><p>Amount to receive <strong>{btcPreview.amount} sats</strong></p><button type="button" className="button-primary" onClick={startBtc} disabled={busy !== ''}>{busy === 'btc' ? 'Requesting signature…' : 'Confirm and request signature'}</button><button type="button" className="withdrawal-text-button" onClick={() => setBtcPreview(null)} disabled={busy !== ''}>Cancel preview</button></div>}
        {canWithdraw && btcReady && <button type="button" className="button-primary" onClick={() => broadcast(btcRecord)} disabled={busy !== ''}>{busy === 'broadcast-Bitcoin' ? 'Broadcasting…' : 'Broadcast signed BTC withdrawal'}</button>}
      </article>
    </div>
    <p className="withdrawal-footnote">Each chain is prepared separately. If a signature request is pending, do not start another one. Activity keeps the reported transaction hash.</p>
  </section>;
}

function Dashboard({ account, wallet, status, vaultId, loading, checkingIn, error, actionError, depositAddresses, depositBalances, refreshingBalances, addressError, onReload, onRefreshBalances, onDisconnect, onSetupComplete, onCheckIn }) {
  const [activeSection, setActiveSection] = useState('overview');
  const [navigationOpen, setNavigationOpen] = useState(false);
  const state = status ? displayState(status.state) : null;
  const ownerMismatch = status && status.owner_id && status.owner_id !== account;
  const canCheckIn = status && !ownerMismatch && ['Active', 'Warning', 'Grace'].includes(state.raw);
  const canOpenWithdrawal = status && canOwnerWithdraw(state.raw, ownerMismatch);
  const destinations = status?.heirs || [];
  const completedEthSlots = destinations.filter((slot) => enumValue(slot.status) === 'Done').length;
  const distributionComplete = state?.raw === 'Claimable' && destinations.length > 0 &&
    completedEthSlots === destinations.length && enumValue(status?.btc_slot?.status) === 'Done';
  const visibleState = distributionComplete ? 'Distribution complete' : state?.label;
  const sections = [
    { id: 'overview', label: 'Overview' },
    { id: 'plan', label: 'Plan' },
    { id: 'withdraw', label: 'Withdraw' },
    { id: 'activity', label: 'Activity' },
  ];
  const title = { overview: 'Your backup at a glance', plan: 'Plan and destinations', withdraw: 'Withdraw assets', activity: 'Recent activity' }[activeSection];

  return <div className="dashboard-page">
    <header className="dashboard-nav"><a className="wordmark dashboard-wordmark" href="#top"><Mark /><span>ember<small>PROTOCOL</small></span></a><div className="dashboard-nav-center"><span className="testnet-indicator" /> NEAR TESTNET <span className="nav-divider">/</span> YOUR PLAN</div><div className="wallet-menu"><span className="wallet-ident"><i />{shortAccount(account)}</span><button onClick={onDisconnect}>Disconnect</button></div></header>
    <main className="dashboard-content">
      <div className="dashboard-heading"><div><span className="eyebrow-label">YOUR BACKUP PLAN</span><h1>{title}</h1><p>Connected as <code>{account}</code></p></div><button className="reload-button" onClick={onReload} disabled={loading}>{loading ? 'Refreshing…' : 'Refresh plan'}</button></div>
      {status && <div className="dashboard-view-navigation">
        <button className="dashboard-menu-toggle" type="button" aria-expanded={navigationOpen} aria-controls="dashboard-view-links" onClick={() => setNavigationOpen((open) => !open)}>{navigationOpen ? 'Close menu' : '☰ Menu'}</button>
        <nav id="dashboard-view-links" className={`dashboard-view-links${navigationOpen ? ' is-open' : ''}`} aria-label="Dashboard sections">
          {sections.map((section) => <button key={section.id} type="button" className={activeSection === section.id ? 'is-active' : ''} aria-current={activeSection === section.id ? 'page' : undefined} onClick={() => { setActiveSection(section.id); setNavigationOpen(false); }}>{section.label}</button>)}
        </nav>
      </div>}
      {error && <div className="dashboard-alert" role="alert"><strong>Could not load this plan.</strong> {error}<button onClick={onReload}>Try again</button></div>}
      {actionError && <div className="dashboard-alert" role="alert"><strong>Check-in failed.</strong> {actionError}</div>}
      {loading && <div className="dashboard-loading"><span className="loading-ring" />Loading plan from NEAR Testnet…</div>}
      {!loading && status && <>
        {ownerMismatch && <div className="account-note">This contract is owned by <code>{status.owner_id}</code>. You are connected as <code>{account}</code>. This dashboard is read-only.</div>}
        {activeSection === 'overview' && <>
          <section className={`dashboard-state-card${distributionComplete ? ' distribution-is-complete' : ''}`}>
            <div className="state-card-top"><div><span className="eyebrow-label">{distributionComplete ? 'DELIVERY STATUS' : 'PLAN STATUS'}</span><div className={`status-label status-${distributionComplete ? 'complete' : state.raw.toLowerCase()}`}><i />{visibleState}</div>{distributionComplete && <p className="distribution-status-note">All destinations are marked Delivered (reported).</p>}</div><span className="read-only-badge">LIVE ON TESTNET</span></div>
            <div className="state-card-bottom"><div><span className="dashboard-stat-label">{distributionComplete ? 'Contract phase' : 'Time to next state'}</span><strong>{distributionComplete ? 'Ready for distribution' : formatTime(status.seconds_until_claimable)}</strong></div><div><span className="dashboard-stat-label">Check-in interval</span><strong>{Number(status.heartbeat_interval_sec) / 86400} days</strong></div><div><span className="dashboard-stat-label">Grace period</span><strong>{Number(status.grace_period_sec) / 86400} days</strong></div><button className="disabled-checkin" onClick={onCheckIn} disabled={!canCheckIn || loading || checkingIn} title={canCheckIn ? 'Submit a check-in from your connected NEAR account' : 'Check-in is available before the plan becomes ready for distribution'}>{checkingIn ? 'Checking in…' : 'Check in'}</button></div>
          </section>
          <div className="overview-withdraw-action">
            <button type="button" className="overview-withdraw-button" onClick={() => setActiveSection('withdraw')} disabled={!canOpenWithdrawal} aria-describedby="overview-withdraw-help">
              Withdraw assets <Arrow />
            </button>
            <p id="overview-withdraw-help">{ownerMismatch ? 'Only the account that owns this plan can withdraw.' : canOpenWithdrawal ? 'Available before the plan reaches Ready for distribution.' : 'Withdrawals are unavailable after the plan reaches Ready for distribution.'}</p>
          </div>
          <DepositAddresses addresses={depositAddresses} balances={depositBalances} refreshing={refreshingBalances} onRefresh={onRefreshBalances} addressError={addressError} />
          <button className="text-link dashboard-view-cta" type="button" onClick={() => setActiveSection('plan')}>View plan and destinations <Arrow /></button>
        </>}
        {activeSection === 'plan' && <>
          <section className="dashboard-state-card plan-summary-card"><div className="state-card-top"><div><span className="eyebrow-label">CURRENT SCHEDULE</span><div className="status-label">{Number(status.heartbeat_interval_sec) / 86400} day check-in</div></div><span className="read-only-badge">ACTIVE PLAN</span></div><div className="state-card-bottom plan-schedule-stats"><div><span className="dashboard-stat-label">Grace period</span><strong>{Number(status.grace_period_sec) / 86400} days</strong></div><div><span className="dashboard-stat-label">Next step</span><strong>{formatTime(status.seconds_until_claimable)}</strong></div></div></section>
          {status.pending_plan_change && <section className="pending-plan-card"><span className="eyebrow-label">CHANGE SCHEDULED</span><h2>Plan update pending</h2><p>This update is scheduled to take effect on {formatTimestamp(status.pending_plan_change.effective_at_ns)}.</p></section>}
          <div className="dashboard-section-title"><div><span className="eyebrow-label">YOUR SELECTED DESTINATIONS</span><h2>Where the plan points</h2></div><span className="destination-count">{destinations.length} {destinations.length === 1 ? 'destination' : 'destinations'}</span></div>
          <DestinationList destinations={destinations} />
          <section className="dashboard-tech-note"><span className="near-trust-icon">N</span><p><strong>Coordinated by NEAR Chain Signatures</strong><span>The plan is stored on NEAR Testnet and routes to Bitcoin and Ethereum.</span></p><span className="dashboard-chain-tags"><i>BTC</i><i>ETH</i></span></section>
        </>}
        {activeSection === 'withdraw' && <WithdrawalPanel account={account} wallet={wallet} status={status} state={state} ownerMismatch={ownerMismatch} addresses={depositAddresses} vaultId={vaultId} onReload={onReload} />}
        {activeSection === 'activity' && <>
          <section className="activity-panel"><div className="activity-panel-heading"><span className="eyebrow-label">WITHDRAWALS</span><h2>Withdrawal history</h2></div>{status.withdrawal_records?.length ? <div className="activity-list">{[...status.withdrawal_records].reverse().map((record, index) => <article className="activity-row" key={`${record.timestamp_ns}-${index}`}><div><strong>{enumValue(record.chain)} withdrawal</strong><span>{formatTimestamp(record.timestamp_ns)} · {withdrawalDestination(record)}</span></div><div className="activity-row-result"><strong>{formatWithdrawalAmount(record)}</strong><span className={`slot-status slot-${enumValue(record.status).toLowerCase()}`}>{enumValue(record.status)}</span></div>{record.reported_tx_hash && <><code className="activity-hash">{record.reported_tx_hash}</code><ReportedConfirmation chain={enumValue(record.chain)} txHash={record.reported_tx_hash} /></>}</article>)}</div> : <div className="dashboard-empty"><h3>No withdrawals recorded</h3><p>Withdrawal activity will appear here.</p></div>}</section>
          <section className="activity-panel"><div className="activity-panel-heading"><span className="eyebrow-label">DISTRIBUTION</span><h2>Delivery reports</h2></div><div className="activity-list">{destinations.map((slot, index) => <article className="activity-row" key={`eth-${index}`}><div><strong>Ethereum · destination {index + 1}</strong><span>{slot.heir?.ethereum_address}</span></div><div className="activity-row-result"><span className={`slot-status slot-${enumValue(slot.status).toLowerCase()}`}>{deliveryLabel(slot.status)}</span></div>{slot.reported_tx_hash && <><code className="activity-hash">{slot.reported_tx_hash}</code><ReportedConfirmation chain="Ethereum" txHash={slot.reported_tx_hash} /></>}</article>)}<article className="activity-row"><div><strong>Bitcoin distribution</strong><span>Bitcoin Testnet</span></div><div className="activity-row-result"><span className={`slot-status slot-${enumValue(status.btc_slot?.status).toLowerCase()}`}>{deliveryLabel(status.btc_slot?.status)}</span></div>{status.btc_slot?.reported_tx_hash && <><code className="activity-hash">{status.btc_slot.reported_tx_hash}</code><ReportedConfirmation chain="Bitcoin" txHash={status.btc_slot.reported_tx_hash} /></>}</article></div></section>
        </>}
      </>}
      {!loading && !status && <SetupForm account={account} wallet={wallet} depositAddresses={depositAddresses} onComplete={onSetupComplete} />}
      <footer className="dashboard-footer"><span>Ember Protocol · Testnet preview</span><a href="https://explorer.testnet.near.org/accounts/3mber.testnet" target="_blank" rel="noreferrer">View contract on NEAR Explorer <Arrow diagonal /></a></footer>
    </main>
  </div>;
}

function App() {
  const connectorRef = useRef(null);
  const [account, setAccount] = useState(null);
  const [wallet, setWallet] = useState(null);
  const [status, setStatus] = useState(null);
  const [depositAddresses, setDepositAddresses] = useState(null);
  const [depositBalances, setDepositBalances] = useState({ ethereum: {}, bitcoin: {} });
  const [refreshingBalances, setRefreshingBalances] = useState(false);
  const balanceRequestId = useRef(0);
  const [addressError, setAddressError] = useState('');
  const [loading, setLoading] = useState(false);
  const [checkingIn, setCheckingIn] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState('');
  const [actionError, setActionError] = useState('');
  const [screen, setScreen] = useState('landing');
  const vaultId = VAULT_ACCOUNT_ID;
  const [restoringWallet, setRestoringWallet] = useState(true);

  const loadDepositBalances = useCallback(async (addresses) => {
    if (!addresses?.ethereum?.address || !addresses?.bitcoin?.address) return;
    const requestId = ++balanceRequestId.current;
    setRefreshingBalances(true);
    setDepositBalances((current) => ({
      ethereum: { ...current.ethereum, loading: true, error: null },
      bitcoin: { ...current.bitcoin, loading: true, error: null },
    }));
    const [ethereum, bitcoin] = await Promise.allSettled([
      readEthereumBalance(addresses.ethereum.address),
      readBitcoinBalance(addresses.bitcoin.address),
    ]);
    if (requestId !== balanceRequestId.current) return;
    const asOf = Date.now();
    setDepositBalances((current) => ({
      ethereum: ethereum.status === 'fulfilled'
        ? { balance: ethereum.value, asOf, loading: false, error: null }
        : { ...current.ethereum, loading: false, error: ethereum.reason?.message || 'Could not load the Ethereum balance.' },
      bitcoin: bitcoin.status === 'fulfilled'
        ? { balance: bitcoin.value, asOf, loading: false, error: null }
        : { ...current.bitcoin, loading: false, error: bitcoin.reason?.message || 'Could not load the Bitcoin balance.' },
    }));
    setRefreshingBalances(false);
  }, []);

  const loadStatus = useCallback(async (ownerId) => {
    if (!ownerId) return null;
    setLoading(true);
    setError('');
    try {
      const nextStatus = await readVaultStatus(ownerId, vaultId);
      setStatus(nextStatus);
      return nextStatus;
    }
    catch (cause) { setStatus(null); setError(cause instanceof Error ? cause.message : 'NEAR Testnet RPC request failed.'); return null; }
    finally { setLoading(false); }
  }, [vaultId]);

  const enterDashboard = useCallback(async (nextWallet, nextAccount) => {
    balanceRequestId.current += 1;
    setWallet(nextWallet);
    setAccount(nextAccount);
    setScreen('dashboard');
    setDepositAddresses(null);
    setDepositBalances({ ethereum: {}, bitcoin: {} });
    setRefreshingBalances(false);
    setAddressError('');
    await loadStatus(nextAccount);
    try {
      const addresses = await deriveVaultDepositAddresses(nextAccount, vaultId);
      setDepositAddresses(addresses);
      void loadDepositBalances(addresses);
    } catch (cause) {
      setAddressError(cause instanceof Error ? cause.message : 'Could not derive the testnet deposit addresses.');
    }
  }, [loadDepositBalances, loadStatus, vaultId]);

  useEffect(() => {
    let cancelled = false;
    const restoreWallet = async () => {
      try {
        const connectorInstance = await getConnector();
        if (cancelled) return;
        connectorRef.current = connectorInstance;
        await connectorInstance.whenManifestLoaded;
        const connection = await connectorInstance.getConnectedWallet();
        const accountId = connection?.accounts?.[0]?.accountId;
        if (!cancelled && connection?.wallet && accountId) {
          await enterDashboard(connection.wallet, accountId);
        }
      } catch {
        // No saved wallet session is normal; leave the connect screen available.
      } finally {
        if (!cancelled) setRestoringWallet(false);
      }
    };
    restoreWallet();
    return () => { cancelled = true; };
  }, [enterDashboard]);

  const connect = async () => {
    setConnecting(true);
    setError('');
    try {
      const connectorInstance = await getConnector();
      connectorRef.current = connectorInstance;
      await connectorInstance.whenManifestLoaded;
      await connectorInstance.connect();
      const connection = await connectorInstance.getConnectedWallet();
      const accountId = connection?.accounts?.[0]?.accountId;
      if (!connection?.wallet || !accountId) throw new Error('The wallet did not return a NEAR account.');
      await enterDashboard(connection.wallet, accountId);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Wallet connection was not completed.');
    } finally { setConnecting(false); }
  };

  const checkIn = async () => {
    if (!wallet || !account || !vaultId) return;
    setCheckingIn(true);
    setActionError('');
    try {
      await pingVault(wallet, account, vaultId);
      await loadStatus(account);
    } catch (cause) {
      setActionError(cause instanceof Error ? cause.message : 'The wallet did not complete the check-in.');
    } finally {
      setCheckingIn(false);
    }
  };

  const disconnect = async () => {
    balanceRequestId.current += 1;
    try { if (wallet && connectorRef.current) await connectorRef.current.disconnect(wallet); }
    finally { setWallet(null); setAccount(null); setStatus(null); setDepositAddresses(null); setDepositBalances({ ethereum: {}, bitcoin: {} }); setRefreshingBalances(false); setAddressError(''); setError(''); setActionError(''); setScreen('landing'); }
  };

  return screen === 'dashboard' && account
    ? <Dashboard account={account} wallet={wallet} status={status} vaultId={vaultId} loading={loading} checkingIn={checkingIn} error={error} actionError={actionError} depositAddresses={depositAddresses} depositBalances={depositBalances} refreshingBalances={refreshingBalances} addressError={addressError} onReload={() => loadStatus(account)} onRefreshBalances={() => loadDepositBalances(depositAddresses)} onDisconnect={disconnect} onSetupComplete={() => loadStatus(account)} onCheckIn={checkIn} />
    : <Landing onConnect={connect} connecting={connecting} restoringWallet={restoringWallet} error={error} />;
}

createRoot(document.getElementById('root')).render(<React.StrictMode><App /></React.StrictMode>);
