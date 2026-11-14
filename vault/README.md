# Ember NEAR contract

This crate implements Ember's shared NEAR Testnet contract. One contract stores a separate vault record for each owner; users do not deploy an individual contract per plan.

## Responsibilities

- Store each owner's receiving plan, shares, check-in timing, keeper authorization, and registered withdrawal addresses.
- Derive an owner-specific signing path and request threshold signatures from the configured NEAR Chain Signatures signer.
- Enforce lifecycle, permissions, fee limits, one outstanding action per chain, retry timing, and the 14-day delay for plan changes.
- Expose owner count/list views for the shared keeper and a status view for the frontend.

The contract does not directly hold Bitcoin or Ethereum balances. Those assets are held at addresses derived for each owner and are spent by signed transactions on Bitcoin Testnet and Ethereum Sepolia.

## Build and test

Run unit tests from this directory:

```bash
cargo test
```

Build the WebAssembly contract and ABI on Linux with:

```bash
cargo near build non-reproducible-wasm --locked
```

The repository's **Contract build (no deploy)** workflow runs this build on GitHub's Linux runner. A successful build does not deploy the contract or migrate existing on-chain state.

## Deployment caution

The deployed contract account is shared by all owner records. Changing Rust state layout requires a compatible migration or a fresh contract deployment; compiling the new crate alone does not update any deployed code or existing records. Confirm the target account, storage layout, and migration plan before deploying.

The root [README](../README.md) describes the frontend and keeper flow. The public contract methods and test cases are documented alongside their implementation in `src/lib.rs`.
