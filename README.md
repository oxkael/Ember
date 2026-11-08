## Contents
- What Ember is
- What Near technology makes possible
- Roadmap
- Step by step testing guide


## 1. What Ember is

Self-custody gives people control over their digital assets, but it can also leave those assets vulnerable to a simple problem: the owner may become unable to manage them which has been a disadvantage to it. However, Ember powered by Near chain sigantures is solving that.

**Ember Protocol** is an automated backup and recovery plan for digital assets across blockchains that Near chain signature supports. An owner sets receiving addresses, how assets should be divided, and how often they’ll check in. If they miss a check-in and the configured waiting periods pass, Ember can start sending assets according to that plan.
That can help when someone loses access to their usual wallet, is away for an extended period, faces a medical emergency, or dies. Ember helps prevent assets from being left inaccessible indefinitely or permanently lost due to absence of owner.

## What NEAR technology makes possible

Ember uses a NEAR smart contract to hold each owner’s plan and track its check-in timer. When the plan is ready, a shared keeper can start the process. NEAR Chain Signatures use threshold MPC to derive external-chain addresses and sign transactions for the contract’s requested transfers. The keeper coordinates the process; it does not hold the Bitcoin or Ethereum signing keys.
This lets Ember send native assets on chains such as Bitcoin and Ethereum without wrapping them or deploying a separate Ember contract on every destination chain. The current demo is on testnets: Bitcoin Testnet and Ethereum Sepolia.

## Roadmap 

**NEAR** Chain Signatures support across ECDSA-based chains, including Bitcoin, Ethereum, Cosmos chains, Dogecoin, and XRP Ledger. NEAR has also announced EdDSA signing for networks including Solana, TON, Stellar, Sui, and Aptos. (NEAR Chain Signatures launch, NEAR EdDSA announcement)
That cryptographic support gives Ember candidates to build toward.


## Ember test workflow

This is a testnet-only exercise. Testnet BTC and ETH have no real-world value. Don’t enter a seed phrase or private key into Ember or a faucet.
Faucet links
- Bitcoin Testnet faucet: https://coinfaucet.eu/en/btc-testnet/ -- Choose Bitcoin testnet 3 and drip
- NEAR Testnet faucet: https://docs.near.org/getting-started/faucet
- Ethereum Sepolia faucet: https://cloud.google.com/application/web3/faucet

## 1. Prepare test wallets
- Connect a NEAR Testnet wallet with enough test NEAR for setup and check-in transactions.
- Have an Ethereum Sepolia receiving address.
- Have a Bitcoin Testnet receiving address starting with tb1q. Use Unisat wallet preferably
- Have a NEAR account for the manual fallback. If you plan to test that fallback, fund it with test NEAR too.

## 2. Connect to Ember and fund its addresses
1. Open the Ember test site and connect the NEAR Testnet wallet.
2. Ember should show the Bitcoin and Ethereum vault addresses derived through NEAR Chain Signatures.
3. Copy those vault addresses into the matching faucets above and request test funds.
4. Wait for the deposits to confirm, then refresh Ember and check that both balances appear.

## 3. Create a plan
1. Enter the receiving address details and choose shares totaling 100%.
2. For a first test, use one receiving address at 100%. Then make a separate test with two addresses, such as 70/30.
3. Choose a short check-in interval and grace period for the demo.
4. Review the addresses, shares, and estimates, then approve the setup transaction in your NEAR wallet.
5. Check in as prompted and confirm Ember shows the expected plan status.

## Automatic distribution
1. After the check-in and grace periods expire, Ember’s scheduled keeper detects that the plan is ready and starts the transfers automatically.
2. Allow time for the keeper’s next scheduled check and for the Bitcoin and Ethereum Testnet transactions to confirm. Then check Ember’s activity and the receiving wallets to confirm the transfers arrived.
