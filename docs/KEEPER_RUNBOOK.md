# Testnet keeper runbook

The shared keeper runs from GitHub Actions. The workflow keeps a manual **Run workflow** option and is configured to poll every five minutes. GitHub scheduled workflows run only from the repository's default branch, and GitHub may delay them. This is useful for the testnet demo; it is not a precise scheduler.

## GitHub configuration

Repository variables:

- `EMBER_CONTRACT_ID`: deployed shared Ember contract account ID.
- `EMBER_KEEPER_ACCOUNT_ID`: dedicated shared keeper NEAR account ID.

Repository secret:

- `EMBER_KEEPER_PRIVATE_KEY`: keeper NEAR function-call key. Keep it in GitHub Secrets; never commit or print it.

The function-call key must target the configured Ember contract and include `claim`, `claim_btc`, `mark_done`, and `mark_btc_done`. `close_outstanding` is allowed by the script's key validator. It must not be a full-access key. The keeper account also needs NEAR for the keeper's own transactions.

Every owner plan must explicitly authorize the shared keeper account. If the keeper list is empty, the contract leaves recipient-driven claims available; the automated workflow skips that owner.

## Automatic and manual operation

After the workflow is present on the default branch and GitHub variables and secret are configured, scheduled runs scan the registered owner list and process up to 20 eligible records per run. To test a specific owner, open **Actions → Ember Keeper (testnet) → Run workflow** and enter the public owner account ID in `owner_id`. An empty input scans authorized records.

The run checks contract status, derives that owner's signing addresses, and skips non-Claimable or already completed records. Empty chain balances are skipped. It requests a NEAR Chain Signatures signature through the contract, rebuilds the external transaction and checks its hash against contract state before broadcasting. A transaction that is broadcast but not yet confirmed stays Signed; the next scheduled run checks it again. It reports a transaction hash as Done only after the public endpoint reports confirmation.

## Retry and ordering behavior

- At most one Ethereum chain action and one Bitcoin chain action may be outstanding for a record.
- ETH claims use one recipient slot per transaction. The keeper continues to the next unfinished ETH slot after the prior transfer confirms and is recorded Done.
- BTC distribution uses one confirmed UTXO and produces outputs for all configured recipients in one transaction.
- ETH signature retries reuse the slot's nonce and wait at least 60 seconds; the contract rejects callbacks from an older attempt.
- BTC claim retries use the same recorded UTXO and require a higher fee rate after the timeout.
- If an action is pending or the signature has not settled, the next scheduled run can continue from stored contract state without waiting in a long-running job for external confirmation.
- Errors are logged per chain and per owner. One failing public endpoint should not prevent the run from checking later owners; the workflow reports a failed run if any owner or chain check fails.

## Diagnosing a missed recipient

For the affected owner, compare `status().heirs` in order. Record each slot's status, `last_nonce`, `last_gas_price_wei`, `signed_hash`, `reported_tx_hash`, and the transaction's explorer result. Then inspect the keeper Actions log for the `ETH:` result for that owner. A run that prints `ETH delivered: <hash>` should be followed by a fresh status read before expecting the next slot. Preserve the raw error and transaction hash; do not rotate keys or submit another claim with a different nonce until the outstanding action is understood.

The scheduled job depends on public NEAR RPC, Sepolia RPC, and the Bitcoin Testnet indexer. It may be delayed by GitHub scheduling, public endpoint outages/rate limits, insufficient keeper NEAR, missing authorization, or invalid secrets. It is a testnet automation path and should be monitored during a judging/demo window.
