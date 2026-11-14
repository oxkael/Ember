# Ember Protocol

**Ember is an automated backup plan for digital assets.** An owner chooses receiving addresses and shares, then checks in on a schedule they control. If the check-in and grace periods expire, an authorized keeper can start the plan's testnet transfers.

Ember demonstrates NEAR Chain Signatures coordinating native Bitcoin and Ethereum transactions from one NEAR contract. The current demo targets **Bitcoin Testnet** and **Ethereum Sepolia**; it does not move mainnet funds.

[Demo video: DEMO VIDEO LINK] | [Announcement post: X POST LINK]

## Why Ember

Self-custody gives people control over their digital assets, but it can also leave those assets vulnerable if the owner becomes unable to manage them. Ember is an automated backup and recovery plan: an owner sets receiving addresses, how assets should be divided, and how often they will check in. If they miss a check-in and the configured waiting periods pass, Ember can start sending assets according to that plan.

That can help when someone loses access to their usual wallet, is away for an extended period, faces a medical emergency, or dies. Ember helps prevent assets from being left inaccessible indefinitely or permanently lost due to the owner's absence.

An estimate widely attributed to Chainalysis puts lost or stranded Bitcoin at roughly 3.7 million coins. Estimates range from roughly 11% to 23% depending on method; they count multiple causes, not death alone, and the original Chainalysis figure dates from 2020. See sources at the end.

## What NEAR technology makes possible

Ember uses a NEAR smart contract to hold each owner's plan and track its check-in timer. When a plan is ready, a shared keeper can start the process. NEAR Chain Signatures use threshold MPC to derive external-chain addresses and sign transactions for the contract's requested transfers. The keeper coordinates the process; it does not hold the Bitcoin or Ethereum signing keys.

This lets Ember send native assets on chains such as Bitcoin and Ethereum without wrapping them or deploying a separate Ember contract on every destination chain. One NEAR account and derivation path produce the same addresses, so the Bitcoin and Ethereum addresses can be shown before a plan is created. The current demo is on testnets: Bitcoin Testnet and Ethereum Sepolia.

NEAR describes Chain Signatures as letting NEAR accounts, including smart contracts, sign transactions for other blockchains. This gives Ember a shared control plane for its cross-chain plan:

- One check-in plan can coordinate assets on multiple supported chains.
- Assets remain native to their networks; Ember does not wrap or bridge them.
- Threshold signing is provided by NEAR's MPC network.

## How it works

1. The browser derives owner-specific Bitcoin and Ethereum addresses from the NEAR Chain Signatures public key, the shared contract ID, and a path tied to the connected NEAR account.
2. The owner funds those addresses and submits a plan with receiving addresses, shares, a check-in interval, a grace period, and optionally the shared keeper account.
3. The NEAR contract stores an independent record for each owner and evaluates its lifecycle from NEAR block time.
4. When a record is `Claimable`, the scheduled keeper checks its balances, asks the contract to request MPC signatures, reconstructs and checks the signed transaction, broadcasts to the destination testnet, and records the reported transaction hash after confirmation.

The keeper uses a NEAR key only for its authorized calls to the Ember contract. Bitcoin distribution is one transaction with an output for each configured receiving address; Ethereum distribution processes one receiving-address slot at a time.

The contract also supports owner withdrawals to registered addresses before `Claimable`, delayed receiving-address or plan changes, and check-ins. Plan changes have a 14-day delay. See the contract and frontend for the current method behavior and limitations.

## Roadmap

NEAR Chain Signatures support ECDSA-based chains including Bitcoin, Ethereum, Cosmos chains, Dogecoin, and XRP Ledger. NEAR has also announced EdDSA signing for networks including Solana, TON, Stellar, Sui, and Aptos. These are candidates for future Ember support, subject to confirming each network's signing and transaction workflow and testing it. See the NEAR Chain Signatures launch and NEAR EdDSA announcement sources.

## Try the judge preview

Choose **Judge demo** on the landing page to open a wallet-free, read-only walkthrough. It uses sample values and addresses only; it does not read a real contract, connect a wallet, or send transactions.

To create or manage a live testnet plan, connect a NEAR Testnet wallet. The wallet is required to authorize setup, check-ins, withdrawals, and other contract actions. Do not enter a seed phrase or private key into Ember.

## Test workflow

This is a testnet-only exercise. Testnet BTC and ETH have no real-world value. Never enter a seed phrase or private key into Ember or a faucet.

Faucets:

- Bitcoin Testnet faucet: <https://coinfaucet.eu/en/btc-testnet/> — choose Bitcoin Testnet 3.
- NEAR Testnet faucet: <https://docs.near.org/getting-started/faucet>
- Ethereum Sepolia faucet: <https://cloud.google.com/application/web3/faucet>

### 1. Prepare test wallets

- Connect a NEAR Testnet wallet with enough test NEAR for setup and check-in transactions.
- Have an Ethereum Sepolia receiving address.
- Have a Bitcoin Testnet receiving address starting with `tb1q` (UniSat is one wallet option).
- Have a NEAR account for manual fallback claims. Fund it with test NEAR if you plan to test that fallback.

### 2. Connect to Ember and fund its addresses

1. Open the Ember test site and connect the NEAR Testnet wallet.
2. Ember should show the Bitcoin and Ethereum vault addresses derived through NEAR Chain Signatures.
3. Copy those vault addresses into the matching faucets above and request test funds.
4. Wait for deposits to confirm, then refresh Ember and check that both balances appear.

<img width="1904" height="717" alt="Ember vault addresses" src="https://github.com/user-attachments/assets/d631c107-a7ec-47b5-9360-0fd9b79053f2" />

### 3. Create a plan

1. Enter the receiving address details and choose shares totaling 100%.
2. For a first test, use one receiving address at 100%. Then make a separate test with two addresses, such as 70/30.
3. Choose short check-in and grace periods for the demo.
4. Review the addresses, shares, and estimates, then approve the setup transaction in your NEAR wallet.
5. Check in as prompted and confirm Ember shows the expected plan status.

<img width="1904" height="900" alt="Ember plan setup" src="https://github.com/user-attachments/assets/c1c03231-ca4a-4aee-945e-34512c14dbe8" />

### 4. Automatic distribution

After the check-in and grace periods expire, the scheduled keeper detects eligible records and starts transfers automatically. Allow time for the next keeper check and for Bitcoin Testnet and Ethereum Sepolia transactions to confirm. Check Ember's activity and the receiving wallets to confirm the transfers arrived.

The scheduled workflow runs every five minutes from the repository's **default branch**. GitHub may delay scheduled jobs, so this is a polling target, not an exact execution time. A manual run is also available in GitHub Actions. Public RPC and indexer availability can delay balance reads, signing, broadcast, or confirmation.

## Automatic keeper configuration

Configure these GitHub repository variables:

- `EMBER_CONTRACT_ID` — the deployed shared contract account ID.
- `EMBER_KEEPER_ACCOUNT_ID` — the dedicated shared keeper NEAR account ID.

Configure this GitHub repository secret:

- `EMBER_KEEPER_PRIVATE_KEY` — the keeper's NEAR function-call key, scoped to the Ember contract and only the keeper methods. Never use an owner or contract-account key here.

Each owner's plan must explicitly authorize that keeper account. The keeper account needs NEAR for its own transaction fees. The contract pays the MPC signing deposit. For setup and operational details, see [docs/KEEPER_RUNBOOK.md](docs/KEEPER_RUNBOOK.md).

## Develop and verify

Install Node.js 22 or later and Rust. From the repository root:

```bash
npm ci
npm test
npm run build
```

Run contract tests from `vault/`:

```bash
cargo test
```

The Linux **Contract build (no deploy)** GitHub Actions workflow runs `cargo near build` and ABI generation. Building does not deploy or migrate the live testnet contract.

## Repository map

- `frontend/` — React/Vite landing page, judge demo, wallet connection, dashboard, and contract adapter.
- `vault/` — NEAR Rust contract and unit tests.
- `scripts/keeper-run-now.mjs` — keeper scan, signing orchestration, broadcast, and reporting.
- `scripts/keeper-logic.mjs` and `tests/` — keeper transaction logic and JavaScript tests.
- `.github/workflows/` — testnet keeper automation and contract build workflow.
- `docs/KEEPER_RUNBOOK.md` — keeper configuration and operating notes.

## Other NEAR Hackathon submission

<https://github.com/oxkael/Ajocircle>

## Sources

- Lost-Bitcoin estimates and methodology: [Blockonomi](https://blockonomi.com/what-happens-to-crypto-when-you-die-key-statistics-and-facts/).
- NEAR Chain Signatures: [NEAR documentation](https://docs.near.org/chain-abstraction/chain-signatures).
