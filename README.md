# Ember Protocol

**Ember is an automated backup plan for digital assets.** An owner chooses receiving addresses and shares, then checks in on a schedule they control. If the check-in and grace periods expire, an authorized keeper can start the plan's testnet transfers.

Ember demonstrates NEAR Chain Signatures coordinating native Bitcoin and Ethereum transactions from one NEAR contract. The current demo targets **Bitcoin Testnet** and **Ethereum Sepolia**; it does not move mainnet funds.

## Try the judge preview

Choose **Judge demo** on the landing page to open a wallet-free, read-only walkthrough. It uses sample values and addresses only; it does not read a real contract, connect a wallet, or send transactions.

To create or manage a live testnet plan, connect a NEAR Testnet wallet. The wallet is required to authorize setup, check-ins, withdrawals, and other contract actions. Do not enter a seed phrase or private key into Ember.

## How it works

1. The browser derives owner-specific Bitcoin and Ethereum addresses from the NEAR Chain Signatures public key, the shared contract ID, and a path tied to the connected NEAR account.
2. The owner funds those addresses and submits a plan with receiving addresses, shares, a check-in interval, a grace period, and optionally the shared keeper account.
3. The NEAR contract stores an independent record for each owner and evaluates its lifecycle from NEAR block time.
4. When a record is `Claimable`, the scheduled keeper checks its balances, asks the contract to request MPC signatures, reconstructs and checks the signed transaction, broadcasts to the destination testnet, and records the reported transaction hash after confirmation.

The keeper uses a NEAR key only for its authorized calls to the Ember contract. NEAR Chain Signatures provide the threshold signing path for Bitcoin and Ethereum. Bitcoin distribution is one transaction with an output for each configured receiving address; Ethereum distribution processes one receiving-address slot at a time.

The contract also supports owner withdrawals to registered addresses before `Claimable`, delayed receiving-address or plan changes, and check-ins. Plan changes have a 14-day delay. See the contract and frontend for the current method behavior and limitations.

## Automatic keeper

The GitHub Actions workflow is configured to check for eligible records every five minutes and also supports a manual run. Scheduled workflows run from the repository's **default branch** after the workflow file is merged or pushed there. GitHub may delay scheduled jobs, so the interval is a polling target, not an exact execution time.

Configure these GitHub repository variables:

- `EMBER_CONTRACT_ID` — the deployed shared contract account ID.
- `EMBER_KEEPER_ACCOUNT_ID` — the dedicated shared keeper NEAR account ID.

Configure this GitHub repository secret:

- `EMBER_KEEPER_PRIVATE_KEY` — the keeper's NEAR function-call key, scoped to the Ember contract and only the keeper methods. Never use an owner or contract-account key here.

Each owner's plan must explicitly authorize that keeper account. The keeper account needs NEAR for its own transaction fees. The contract pays the MPC signing deposit. For setup and operational details, see [docs/KEEPER_RUNBOOK.md](docs/KEEPER_RUNBOOK.md).

## Test workflow

1. Use a NEAR Testnet wallet and test-only receiving wallets. Fund the owner with test NEAR for contract calls.
2. Connect to Ember. Copy the derived Bitcoin Testnet and Ethereum Sepolia addresses and fund them using testnet faucets.
3. Add receiving addresses and shares totaling 100%, configure a keeper if you want automation, and choose short test-only timing values.
4. Approve setup and check in. Confirm the contract status and balances in the dashboard.
5. For a distribution test, wait until the configured check-in and grace periods expire. The scheduled keeper polls eligible records; a manual run is also available under GitHub Actions.
6. Check the contract activity and the relevant public testnet explorers. Status text distinguishes reported delivery from independently displayed chain confirmations.

Keep testnet values small. Public RPC and indexer availability can delay balance reads, signing, broadcast, or confirmation.

## Develop and verify

Install Node.js 22 or later and Rust. From the repository root:

```bash
npm ci
npm test
npm run build
```

Run the contract tests from `vault/`:

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

## Roadmap

The current implementation covers NEAR Testnet, Bitcoin Testnet, and Ethereum Sepolia. Other networks are candidates for future support only after their NEAR Chain Signatures signing and transaction workflows are confirmed and tested.
