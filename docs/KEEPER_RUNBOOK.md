# Testnet keeper run

The keeper workflow is manual. Each run processes at most one Claimable owner record authorized for the configured keeper. It checks both chains, requests MPC signatures for eligible claims, reconstructs and verifies each signed transaction against the contract's stored hash, broadcasts it, waits for confirmation, then reports the transaction hash to the contract.

## GitHub configuration

Set these repository variables:

- `EMBER_CONTRACT_ID`: the fresh shared contract account ID.
- `EMBER_KEEPER_ACCOUNT_ID`: the dedicated keeper NEAR account ID.

Set this repository secret:

- `EMBER_KEEPER_PRIVATE_KEY`: the dedicated keeper's NEAR key. Do not use the owner key or contract account key.

Then open **Actions → Keeper Run Now (testnet) → Run workflow**. The workflow may submit NEAR calls and broadcast Bitcoin Testnet or Sepolia transactions for records that have reached Claimable. It does not run on a schedule.

The keeper account must be funded for NEAR transaction fees and must be authorized in each owner's active keeper list. The contract itself pays the MPC signing deposit. The workflow uses public testnet RPC/indexer endpoints; they can rate-limit or become unavailable.

## Current limits

- One owner record is processed per manual run; rerun to continue through additional records.
- Empty chain balances are skipped.
- ETH retries reuse the same nonce with a strictly higher gas price after 60 seconds. The contract callback includes the attempt timestamp, so late signatures from earlier attempts cannot overwrite the latest attempt.
- BTC pending requests retry only with the same UTXO and a strictly higher fee rate after 60 seconds.
- A broadcast transaction is not reported Done until the public endpoint returns a confirmation.
- This workflow has not been exercised against the fresh deployed contract yet.
