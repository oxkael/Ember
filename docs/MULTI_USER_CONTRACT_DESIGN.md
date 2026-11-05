# Multi-user contract design

## Account model

- Deploy one shared Ember contract account.
- Each owner uses their own NEAR account and owns one vault record stored by the shared contract.
- One dedicated keeper service account may be named in each record. The same keeper account ID can serve many records; this does not create a keeper account per user.
- Each recipient keeps their own destination addresses. The contract record stores them with that owner's plan.
- The shared contract account is the Chain Signatures caller. Each record therefore gets stable, distinct derivation paths so users do not share derived BTC or ETH addresses.

## Per-owner state

The contract stores a record keyed by the owner's NEAR account ID. A record contains the current owner's plan, recipient slots, heartbeat and grace timers, claim state, signing paths, Bitcoin sender witness program, and optional keeper account ID. Only the owner can create or change their record or ping its timer. Status reads take an owner account ID; contract state is public on NEAR. Keeper-facing methods take the owner account ID to identify which record to process, then check that record's configured keeper. Recipient fallback calls identify the same owner record and are checked against its recipient list.

No method may use mutable global fields for an owner's timer, recipients, nonce, balance snapshot, signing status, or completion reports. Status and asynchronous MPC callbacks must carry the owner account ID so they cannot read or update another owner's record.

## Chain Signatures paths

Paths are fixed when a record is first created and cannot be changed by later plan updates:

- `ember/v1/<owner-account-id>/ethereum`
- `ember/v1/<owner-account-id>/bitcoin`

The frontend derives the corresponding public keys and addresses using the same shared contract account and paths. It displays the deposit addresses and submits the BTC sender's 20-byte witness program during setup. The contract uses the saved path and witness program when requesting BTC or ETH signatures. The frontend must not invent another path format; contract and frontend share these exact path builders.

## Public method shape

- `setup(...)`: owner is `predecessor_account_id`; creates or updates only that owner's record.
- `ping()`: owner is `predecessor_account_id`; updates only their timer.
- `status(owner_id)`: returns that owner's status or no record.
- `claim(owner_id, heir_index, balance_wei, nonce, gas_price_wei)`: keeper or that record's designated recipient; uses that record's paths and state.
- `claim_btc(owner_id, txid, vout, value_sats, fee_rate_sat_vb)`: same authorization and isolation rules.
- `mark_done(owner_id, heir_index, tx_hash)` and `mark_btc_done(owner_id, tx_hash)`: record an explicitly reported, unverified hash against that record.
- MPC callbacks include `owner_id` plus the relevant slot/attempt identifier and mutate only that record.

The keeper account should use one restricted function-call access key scoped to the shared contract and only the required keeper methods. The contract still performs its own per-record authorization checks.

## Required isolation tests

1. Two owners can create records with different plans and stable, distinct ETH and BTC paths.
2. Updating or pinging one record leaves the other owner's status and timers unchanged.
3. A keeper named by one record cannot claim, mark done, or alter another record.
4. A recipient listed in one record cannot operate another record.
5. ETH and BTC MPC callbacks update only the owner and attempt they were created for.
6. A caller cannot pass an arbitrary signing path or BTC sender witness program to a claim method.

## Deployment boundary

This is a storage and ABI redesign. It requires a fresh contract deployment and a new test vault; the currently deployed single-owner vault is left untouched. Do not deploy until this implementation, its contract tests, frontend adapter, and keeper calls agree on the method arguments.
