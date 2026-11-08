# Ember Protocol — Agent Readme

## Contents

- [What Ember is](#1-what-ember-is)
- [What NEAR technology makes possible](#2-what-near-technology-makes-possible)
- [Roadmap](#3-roadmap)
- [Step-by-step testing guide](#4-step-by-step-testing-guide)

## 1. What Ember is

Self-custody gives people control over their digital assets, but that control can become a vulnerability if the owner can no longer manage their wallet. Ember uses NEAR Chain Signatures to provide a prearranged backup path for those assets.

**Ember Protocol is an automated backup and recovery plan for digital assets across chains supported by NEAR Chain Signatures.** An owner sets receiving addresses, how assets should be divided, and how often they will check in. If check-ins stop and the configured waiting periods pass, Ember can start sending assets according to the owner's plan.

This can help if someone loses access to their usual wallet, is away for an extended period, faces a medical emergency, or dies. Ember's purpose is to help prevent assets from being left inaccessible indefinitely or permanently lost because their owner can no longer act.

## 2. What NEAR technology makes possible

Ember uses a NEAR smart contract to store each owner's plan and track its check-in timer. When a plan is ready, a shared keeper can start its distribution process. NEAR Chain Signatures use threshold multi-party computation (MPC) to derive external-chain addresses and sign transactions requested through the contract. The keeper coordinates the process; it does not hold the Bitcoin or Ethereum signing keys.

This lets Ember send native assets on chains such as Bitcoin and Ethereum without wrapping them or deploying a separate Ember contract on every destination chain. The current demo uses NEAR Testnet, Bitcoin Testnet, and Ethereum Sepolia.

## 3. Roadmap

NEAR has described Chain Signatures for ECDSA-based chains including Bitcoin, Ethereum, Cosmos chains, Dogecoin, and XRP Ledger. NEAR has also announced EdDSA signing for networks including Solana, TON, Stellar, Sui, and Aptos. See the [Chain Signatures launch announcement](https://near.org/blog/chain-signatures-launch-to-enable-transactions-on-any-blockchain-from-a-near-account) and [EdDSA announcement](https://near.org/blog/chain-signatures-adds-ed-dsa-support-cross-chain-signing-for-solana-ton-stellar-sui-aptos).

These capabilities give Ember candidates for future integrations; they do not mean Ember already supports every listed network. Each addition needs its own transaction construction, address and fee handling, broadcasting, and end-to-end testing. A sensible sequence is:

1. Complete and harden the Bitcoin Testnet and Ethereum Sepolia flows.
2. Evaluate XRP Ledger as a next candidate, confirming signer availability and its testnet transaction flow before implementation.
3. Add further ECDSA or EdDSA networks based on user need and readiness, with a dedicated chain adapter for each.

## 4. Step-by-step testing guide

This is a testnet-only exercise. Testnet BTC and ETH have no real-world value. Never enter a seed phrase or private key into Ember or a faucet.

### Faucet links

- **Bitcoin Testnet 3 faucet:** [coinfaucet.eu](https://coinfaucet.eu/en/btc-testnet/) — paste a public Testnet 3 address and request test BTC.
- **NEAR Testnet faucet:** [NEAR faucet instructions](https://docs.near.org/getting-started/faucet).
- **Ethereum Sepolia faucet:** [Google Cloud Web3 faucet](https://cloud.google.com/application/web3/faucet) — follow its current sign-in and eligibility steps.

### 1. Prepare test wallets

- Connect a NEAR Testnet wallet with enough test NEAR for setup and check-in transactions.
- Have an Ethereum Sepolia receiving address.
- Have a Bitcoin Testnet receiving address beginning with `tb1q`. UniSat is one wallet option for Bitcoin Testnet.
- Have a NEAR account for manual fallback. If you plan to test that fallback, fund it with test NEAR too.

### 2. Connect to Ember and fund its addresses

1. Open the Ember test site and connect the NEAR Testnet wallet.
2. Ember should show the Bitcoin and Ethereum vault addresses derived through NEAR Chain Signatures.
3. Copy each vault address into its matching faucet and request test funds.
4. Wait for deposits to confirm, refresh Ember, and check that both balances appear.

### 3. Create a plan

1. Enter the receiving address details and choose shares totaling 100%.
2. For the first test, use one receiving address at 100%. Then run a separate test with two addresses, such as 70/30.
3. Choose a short check-in interval and grace period appropriate for the demo.
4. Review the addresses, shares, and balance estimates, then approve the setup transaction in the NEAR wallet.
5. Check in as prompted and confirm Ember displays the expected plan status.

### 4. Automatic distribution

1. After the check-in and grace periods expire, the scheduled keeper should detect that the plan is ready and start the transfers automatically. The tester should not need to run a GitHub workflow.
2. Allow time for the keeper's next scheduled check and for Bitcoin and Ethereum Testnet transactions to confirm.
3. Check Ember's activity and the receiving wallets. For a two-address test, confirm that both recipients received their expected shares on each chain.

