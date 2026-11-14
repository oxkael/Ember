## Contents
- What Ember is
- What Near technology makes possible
- Roadmap
- Step by step testing guide
-  Other Near Hackathon Submission


## 1. What Ember is

A safety switch for your crypto that you Check in now and then. If check-ins stop, your native Bitcoin and Ethereum go where it is set.

Built on NEAR Chain Signatures. Testnet MVP: Ethereum Sepolia and Bitcoin Testnet.

[Demo video: DEMO VIDEO LINK] | [Announcement post: X POST LINK]

## About a fifth of all Bitcoin is already lost or stranded

An estimate widely attributed to Chainalysis puts it at roughly 3.7 million coins. Some of that is forgotten seed phrases and broken drives. Some of it belongs to owners who died without telling anyone how to reach their wallet.

With a bank account, an executor can find the account, contact the bank and get a court order. On a blockchain there is no such recourse. If nobody has the keys, the assets are gone, and probate cannot recover a key nobody has.

The usual fixes ask you to trust a custodian, hand someone your seed phrase, or build a multisig your family has to operate in the worst week of their lives.

About the number: estimates range from roughly 11% to 23% of all Bitcoin depending on method, they count every cause and not death alone, and the original Chainalysis figure dates from 2020. See sources at the end.
## Roadmap 

**NEAR** Chain Signatures support across ECDSA-based chains, including Bitcoin, Ethereum, Cosmos chains, Dogecoin, and XRP Ledger. NEAR has also announced EdDSA signing for networks including Solana, TON, Stellar, Sui, and Aptos. (NEAR Chain Signatures launch, NEAR EdDSA announcement)
That cryptographic support gives Ember candidates to build toward.

## NEAR Chain Signatures is what makes it possible

NEAR describes Chain Signatures as letting NEAR accounts, including smart contracts, sign transactions for other blockchains such as Bitcoin, Ethereum and Solana. Users control accounts on those chains from their NEAR account, without wrapping or bridging assets.

- That gives Ember three things:

One account, native assets on two chains. The same NEAR account and derivation path always produce the same address, so the vault's Ethereum and Bitcoin addresses exist from the start. Funds sit as real ETH and real BTC, not wrapped tokens.
No single key holder. Signatures are produced jointly by NEAR's MPC network. Nobody holds a private key for these addresses.
One contract holds the rules for every chain. One check-in covers both.


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

<img width="1904" height="717" alt="Screenshot 2026-11-14 134246" src="https://github.com/user-attachments/assets/d631c107-a7ec-47b5-9360-0fd9b79053f2" />


## 3. Create a plan
1. Enter the receiving address details and choose shares totaling 100%.
2. For a first test, use one receiving address at 100%. Then make a separate test with two addresses, such as 70/30.
3. Choose a short check-in interval and grace period for the demo.
4. Review the addresses, shares, and estimates, then approve the setup transaction in your NEAR wallet.
5. Check in as prompted and confirm Ember shows the expected plan status.
<img width="1904" height="900" alt="Screenshot 2026-11-14 134640" src="https://github.com/user-attachments/assets/c1c03231-ca4a-4aee-945e-34512c14dbe8" />


## Automatic distribution
1. After the check-in and grace periods expire, Ember’s scheduled keeper detects that the plan is ready and starts the transfers automatically.
2. Allow time for the keeper’s next scheduled check and for the Bitcoin and Ethereum Testnet transactions to confirm. Then check Ember’s activity and the receiving wallets to confirm the transfers arrived.

## Other Near Hackathon Submission
https://github.com/oxkael/Ajocircle
