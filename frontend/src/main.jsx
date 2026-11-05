import React, { useCallback, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { NEAR_NETWORK, VAULT_ACCOUNT_ID } from './config.js';
import SetupForm from './SetupForm.jsx';
import { deriveVaultDepositAddresses, pingVault, readVaultStatus } from './vaultClient.js';
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

function Landing({ onConnect, connecting, error }) {
  return <div className="landing-page">
    <header className="landing-nav">
      <a className="wordmark" href="#top"><Mark /><span>ember<small>PROTOCOL</small></span></a>
      <nav className="landing-links" aria-label="Page navigation"><a href="#how-it-works">How it works</a><a href="#why-near">Why NEAR</a><a href="#faq">FAQs</a></nav>
      <button className="nav-connect" onClick={onConnect} disabled={connecting}>{connecting ? 'Connecting…' : 'Connect wallet'} <Arrow /></button>
    </header>

    <main id="top">
      <section className="hero-section">
        <div className="hero-copy">
          <div className="hero-eyebrow"><span /> A BACKUP PLAN FOR YOUR DIGITAL ASSETS</div>
          <h1>Your assets deserve<br />a <em>plan B.</em></h1>
          <p className="hero-description">Ember helps you prepare for lost access, emergencies, or time away—with a check-in plan and destinations across Bitcoin and Ethereum.</p>
          <div className="hero-actions"><button className="button-primary" onClick={onConnect} disabled={connecting}>{connecting ? 'Connecting to NEAR…' : 'Connect NEAR wallet'} <Arrow /></button><a className="text-link" href="#how-it-works">Explore how it works <span><Arrow diagonal /></span></a></div>
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
          <details><summary>Is this connected to mainnet?<span className="faq-plus" /></summary><p>No. This frontend currently connects to NEAR Testnet and reads the project's testnet contract. It does not send transactions from this preview.</p></details>
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

function Dashboard({ account, wallet, status, vaultId, loading, checkingIn, error, actionError, depositAddresses, addressError, onReload, onDisconnect, onSetupComplete, onCheckIn }) {
  const state = status ? displayState(status.state) : null;
  const ownerMismatch = status && status.owner_id && status.owner_id !== account;
  const canCheckIn = status && !ownerMismatch && ['Active', 'Warning', 'Grace'].includes(state.raw);
  const destinations = status?.heirs || [];
  return <div className="dashboard-page">
    <header className="dashboard-nav"><a className="wordmark dashboard-wordmark" href="#top"><Mark /><span>ember<small>PROTOCOL</small></span></a><div className="dashboard-nav-center"><span className="testnet-indicator" /> NEAR TESTNET <span className="nav-divider">/</span> YOUR PLAN</div><div className="wallet-menu"><span className="wallet-ident"><i />{shortAccount(account)}</span><button onClick={onDisconnect}>Disconnect</button></div></header>
    <main className="dashboard-content">
      <div className="dashboard-heading"><div><span className="eyebrow-label">YOUR BACKUP PLAN</span><h1>A little more peace of mind.</h1><p>Live read from <code>{vaultId || VAULT_ACCOUNT_ID}</code></p></div><button className="reload-button" onClick={onReload} disabled={loading}>{loading ? 'Refreshing…' : 'Refresh plan'}</button></div>
      <section className="deposit-addresses" aria-labelledby="deposit-addresses-title">
        <div className="deposit-addresses-heading"><div><span className="eyebrow-label">NEAR CHAIN SIGNATURES</span><h2 id="deposit-addresses-title">Your testnet deposit addresses</h2></div><span className="read-only-badge">DERIVED IN BROWSER</span></div>
        <p className="deposit-addresses-intro">These addresses are derived from this shared contract and your connected NEAR account. No setup transaction is needed to derive them.</p>
        {depositAddresses ? <div className="deposit-address-grid">
          <article className="deposit-address-item"><span>ETHEREUM · SEPOLIA</span><code>{depositAddresses.ethereum.address}</code><small>Path: {depositAddresses.ethereum.path}</small></article>
          <article className="deposit-address-item"><span>BITCOIN · TESTNET</span><code>{depositAddresses.bitcoin.address}</code><small>Path: {depositAddresses.bitcoin.path}</small></article>
        </div> : <p className={addressError ? 'deposit-address-error' : 'deposit-address-pending'} role={addressError ? 'alert' : undefined}>{addressError || 'Deriving addresses…'}</p>}
      </section>
      {error && <div className="dashboard-alert" role="alert"><strong>Could not load this plan.</strong> {error}<button onClick={onReload}>Try again</button></div>}
      {actionError && <div className="dashboard-alert" role="alert"><strong>Check-in failed.</strong> {actionError}</div>}
      {loading && <div className="dashboard-loading"><span className="loading-ring" />Loading plan from NEAR Testnet…</div>}
      {!loading && status && <>
        {ownerMismatch && <div className="account-note">This contract is owned by <code>{status.owner_id}</code>. You are connected as <code>{account}</code>. This dashboard is read-only.</div>}
        <section className="dashboard-state-card"><div className="state-card-top"><div><span className="eyebrow-label">PLAN STATUS</span><div className={`status-label status-${state.raw.toLowerCase()}`}><i />{state.label}</div></div><span className="read-only-badge">LIVE ON TESTNET</span></div><div className="state-card-bottom"><div><span className="dashboard-stat-label">Time to next state</span><strong>{formatTime(status.seconds_until_claimable)}</strong></div><div><span className="dashboard-stat-label">Check-in interval</span><strong>{Number(status.heartbeat_interval_sec) / 86400} days</strong></div><div><span className="dashboard-stat-label">Grace period</span><strong>{Number(status.grace_period_sec) / 86400} days</strong></div><button className="disabled-checkin" onClick={onCheckIn} disabled={!canCheckIn || loading || checkingIn} title={canCheckIn ? 'Submit a check-in from your connected NEAR account' : 'Check-in is available before the plan becomes ready for distribution'}>{checkingIn ? 'Checking in…' : 'Check in'}</button></div></section>
        <div className="dashboard-section-title"><div><span className="eyebrow-label">YOUR SELECTED DESTINATIONS</span><h2>Where the plan points</h2></div><span className="destination-count">{destinations.length} {destinations.length === 1 ? 'destination' : 'destinations'}</span></div>
        <section className="dashboard-destinations">{destinations.length ? destinations.map((slot, index) => <article className="dashboard-destination" key={`${slot.heir?.near_account_id || index}`}><div className="destination-avatar">{String(index + 1).padStart(2, '0')}</div><div className="dashboard-destination-info"><span className="eyebrow-label">DESTINATION {String(index + 1).padStart(2, '0')}</span><strong>{slot.heir?.near_account_id || 'NEAR account not set'}</strong><small>{slot.heir?.ethereum_address || 'Ethereum address not set'}</small></div><span className="share-pill">{(Number(slot.heir?.share_basis_points) || 0) / 100}% share</span><span className={`slot-status slot-${String(slot.status || '').toLowerCase()}`}>{slot.status || 'Unknown'}</span></article>) : <div className="dashboard-empty"><h3>No destinations are configured</h3><p>This contract does not have a backup plan set up yet.</p></div>}</section>
        <section className="dashboard-tech-note"><span className="near-trust-icon">N</span><p><strong>Coordinated by NEAR Chain Signatures</strong><span>Plan state is read from NEAR Testnet. Setup and check-ins are signed by your connected wallet.</span></p><span className="dashboard-chain-tags"><i>BTC</i><i>ETH</i></span></section>
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
  const [addressError, setAddressError] = useState('');
  const [loading, setLoading] = useState(false);
  const [checkingIn, setCheckingIn] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState('');
  const [actionError, setActionError] = useState('');
  const [screen, setScreen] = useState('landing');
  const [vaultId, setVaultId] = useState(VAULT_ACCOUNT_ID);

  const loadStatus = useCallback(async (ownerId = account) => {
    setLoading(true);
    setError('');
    try {
      setStatus(await readVaultStatus(ownerId, vaultId));
    }
    catch (cause) { setStatus(null); setError(cause instanceof Error ? cause.message : 'NEAR Testnet RPC request failed.'); }
    finally { setLoading(false); }
  }, [account, vaultId]);

  const enterDashboard = useCallback(async (nextWallet, nextAccount) => {
    setWallet(nextWallet);
    setAccount(nextAccount);
    setScreen('dashboard');
    setDepositAddresses(null);
    setAddressError('');
    await loadStatus(nextAccount);
    try {
      setDepositAddresses(await deriveVaultDepositAddresses(nextAccount, vaultId));
    } catch (cause) {
      setAddressError(cause instanceof Error ? cause.message : 'Could not derive the testnet deposit addresses.');
    }
  }, [loadStatus, vaultId]);

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
    try { if (wallet && connectorRef.current) await connectorRef.current.disconnect(wallet); }
    finally { setWallet(null); setAccount(null); setStatus(null); setDepositAddresses(null); setAddressError(''); setError(''); setActionError(''); setScreen('landing'); }
  };

  return screen === 'dashboard' && account
    ? <Dashboard account={account} wallet={wallet} status={status} vaultId={vaultId} loading={loading} checkingIn={checkingIn} error={error} actionError={actionError} depositAddresses={depositAddresses} addressError={addressError} onReload={loadStatus} onDisconnect={disconnect} onSetupComplete={loadStatus} onCheckIn={checkIn} />
    : <Landing onConnect={connect} connecting={connecting} error={error} />;
}

createRoot(document.getElementById('root')).render(<React.StrictMode><App /></React.StrictMode>);
