import { Account, JsonRpcProvider, KeyPairSigner } from "near-api-js";
import { ethers } from "ethers";
import * as bitcoin from "bitcoinjs-lib";
import { deriveBitcoinTestnetAddress, deriveChildPublicKey, deriveEvmAddress } from "../frontend/src/chainSignatures.js";
import { ripemd160 } from "@noble/hashes/legacy";
import { sha256 } from "@noble/hashes/sha2";
import { calculateBtcOutputs, enumName, hasUnfinishedClaimWork, parseMpcSignature, selectOwnerIds } from "./keeper-logic.mjs";

const contractId = process.env.EMBER_CONTRACT_ID?.trim();
const keeperId = process.env.EMBER_KEEPER_ACCOUNT_ID?.trim();
const rpcUrl = process.env.NEAR_RPC_URL?.trim() || "https://rpc.testnet.near.org";
const evmRpcUrl = process.env.EMBER_SEPOLIA_RPC_URL?.trim() || "https://ethereum-sepolia-rpc.publicnode.com";
const btcApi = process.env.EMBER_BTC_TESTNET_API?.trim() || "https://mempool.space/testnet/api";
const execute = process.env.EMBER_KEEPER_EXECUTE === "1";
const maxRecordsPerRun = Number(process.env.EMBER_KEEPER_MAX_RECORDS || "1");
const requestedOwnerId = process.env.EMBER_KEEPER_OWNER_ID?.trim() || "";
const ETH_CHAIN_ID = 11155111;
const BTC_MAX_FEE_RATE = 200n;
const ETH_ABSOLUTE_FEE_CAP = 2_000_000_000_000_000n;
const REQUIRED_KEEPER_METHODS = ["claim", "claim_btc", "mark_done", "mark_btc_done"];
const ALLOWED_KEEPER_METHODS = new Set([...REQUIRED_KEEPER_METHODS, "close_outstanding"]);

if (!contractId || !keeperId) {
  throw new Error("Set EMBER_CONTRACT_ID and EMBER_KEEPER_ACCOUNT_ID before starting the keeper.");
}
if (!Number.isInteger(maxRecordsPerRun) || maxRecordsPerRun < 1 || maxRecordsPerRun > 20) {
  throw new Error("EMBER_KEEPER_MAX_RECORDS must be an integer between 1 and 20.");
}

const nearProvider = new JsonRpcProvider({ url: rpcUrl });
const evmProvider = new ethers.JsonRpcProvider(evmRpcUrl, ETH_CHAIN_ID);
let keeperAccount;

async function view(methodName, args = {}, accountId = contractId) {
  return nearProvider.callFunction({ contractId: accountId, method: methodName, args });
}

async function waitForStatus(ownerId, predicate, maxAttempts = 45) {
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const status = await view("status", { owner_id: ownerId });
    if (status && predicate(status)) return status;
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  }
  return null;
}

async function assertKeeperKeyScoped() {
  const signer = keeperAccount.getSigner();
  const publicKey = await signer.getPublicKey();
  const accessKey = await nearProvider.viewAccessKey({ accountId: keeperId, publicKey });
  const permission = accessKey.permission;
  if (permission === "FullAccess" || !permission?.FunctionCall) {
    throw new Error("Keeper key must be a function-call access key scoped to the Ember contract.");
  }
  const allowed = permission.FunctionCall;
  if (allowed.receiver_id !== contractId || !allowed.method_names.length ||
      REQUIRED_KEEPER_METHODS.some((method) => !allowed.method_names.includes(method)) ||
      allowed.method_names.some((method) => !ALLOWED_KEEPER_METHODS.has(method))) {
    throw new Error("Keeper function-call key must target the configured Ember contract and allow claim, claim_btc, mark_done, and mark_btc_done.");
  }
}

function stateName(state) {
  return typeof state === "string" ? state : Object.keys(state ?? {})[0] ?? "Unknown";
}

async function readRegistry() {
  const total = await view("owner_count");
  if (!Number.isInteger(total) || total < 0) throw new Error(`Invalid owner_count: ${JSON.stringify(total)}`);
  const owners = [];
  for (let from = 0; from < total; from += 100) {
    const page = await view("list_owners", { from_index: from, limit: 100 });
    if (!Array.isArray(page)) throw new Error("list_owners() did not return an array.");
    owners.push(...page);
  }
  if (owners.length !== total) throw new Error(`Owner registry count mismatch (${owners.length}/${total}).`);
  return owners;
}

async function deriveAddresses(status) {
  const rootKey = await view("public_key", {}, status.mpc_contract);
  const ethKey = deriveChildPublicKey(rootKey, contractId, status.derivation_path);
  const btcKey = deriveChildPublicKey(rootKey, contractId, status.btc_derivation_path);
  return {
    eth: deriveEvmAddress(ethKey),
    btc: deriveBitcoinTestnetAddress(btcKey),
  };
}

function makeEthTransaction(status, slot, balance) {
  const gasLimit = BigInt(status.gas_limit);
  const nonce = BigInt(slot.last_nonce);
  const gasPrice = BigInt(slot.last_gas_price_wei);
  const snapshot = status.snapshot_balance_wei == null ? balance : BigInt(status.snapshot_balance_wei);
  const fee = gasLimit * gasPrice;
  const gross = snapshot * BigInt(slot.heir.share_basis_points) / 10000n;
  if (gross <= fee) throw new Error("ETH share cannot cover the contract-calculated gas fee.");
  const tx = {
    type: 1, chainId: ETH_CHAIN_ID, nonce: Number(nonce), gasPrice,
    gasLimit, to: slot.heir.ethereum_address, value: gross - fee, data: "0x", accessList: [],
  };
  const unsigned = ethers.Transaction.from(tx).unsignedSerialized;
  const unsignedHash = ethers.keccak256(unsigned);
  if (unsignedHash.slice(2).toLowerCase() !== String(slot.signed_hash).replace(/^0x/i, "").toLowerCase()) {
    throw new Error("ETH unsigned transaction hash does not match contract status; refusing to broadcast.");
  }
  return { tx, unsignedHash };
}

async function runEth(ownerId, status, addresses) {
  if (status.withdrawal_records?.some((record) => enumName(record.chain) === "Ethereum" && ["Pending", "Signed"].includes(enumName(record.status)))) {
    return "ETH withdrawal is outstanding; skipped";
  }
  const slotIndex = status.heirs.findIndex((slot) => slot.status === "Signed");
  if (slotIndex < 0) {
    const pendingIndex = status.heirs.findIndex((item) => item.status === "Pending");
    const retry = pendingIndex >= 0;
    if (status.eth_outstanding_nonce != null && !retry) return "ETH chain action already outstanding; skipped";
    const slot = retry ? pendingIndex : status.heirs.findIndex((item) => item.status === "Unsent");
    if (slot < 0) return "ETH slots already processed";
    const balance = await evmProvider.getBalance(addresses.eth);
    if (balance === 0n) return "ETH wallet empty; skipped";
    let nonce;
    if (retry) {
      const lastAttemptNs = BigInt(status.heirs[slot].last_claim_timestamp_ns);
      if (BigInt(status.current_timestamp_ns) - lastAttemptNs < 60_000_000_000n) return "ETH MPC request is still within its 60 second retry window";
      nonce = BigInt(status.heirs[slot].last_nonce);
    } else {
      nonce = BigInt(await evmProvider.getTransactionCount(addresses.eth, "pending"));
      const completedNonces = status.heirs.filter((item) => item.status === "Done").map((item) => BigInt(item.last_nonce));
      const expectedNonce = completedNonces.length ? completedNonces.reduce((a, b) => a > b ? a : b) + 1n : 0n;
      if (nonce !== expectedNonce) return `ETH nonce needs reconciliation (network ${nonce}, contract expects ${expectedNonce}); skipped`;
    }
    const feeData = await evmProvider.getFeeData();
    const previousGasPrice = retry ? BigInt(status.heirs[slot].last_gas_price_wei) : 0n;
    let gasPrice = feeData.gasPrice;
    if (!gasPrice) return "Sepolia RPC did not provide a gas price; skipped";
    if (retry && gasPrice <= previousGasPrice) gasPrice = previousGasPrice + 1n;
    const gasLimit = BigInt(status.gas_limit);
    const pool = status.snapshot_balance_wei == null ? balance : BigInt(status.snapshot_balance_wei);
    const grossShare = pool * BigInt(status.heirs[slot].heir.share_basis_points) / 10000n;
    const fee = gasLimit * gasPrice;
    if (fee > ETH_ABSOLUTE_FEE_CAP || fee >= grossShare || fee * 100n > grossShare * 5n) {
      return "Current ETH gas estimate exceeds the contract fee limits; skipped";
    }
    const claimArgs = { owner_id: ownerId, heir_index: slot, balance_wei: balance.toString(), nonce: nonce.toString(), gas_price_wei: gasPrice.toString() };
    if (!execute) return `ETH ready: would ${retry ? "retry" : "call claim for"} slot ${slot} (nonce ${nonce})`;
    await keeperAccount.callFunction({ contractId, methodName: "claim", args: claimArgs, gas: 300_000_000_000_000n, attachedDeposit: 0n });
    const updated = await waitForStatus(ownerId, (value) => ["Signed", "Unsent"].includes(value.heirs[slot]?.status));
    if (!updated) return "ETH claim submitted; MPC callback has not settled yet, rerun later";
    if (updated.heirs[slot].status !== "Signed") return "ETH MPC signing failed; contract restored the slot for a later attempt";
    return runEth(ownerId, updated, addresses);
  }

  const slot = status.heirs[slotIndex];
  const balance = await evmProvider.getBalance(addresses.eth);
  const { tx } = makeEthTransaction(status, slot, balance);
  const signature = parseMpcSignature(slot.tx_hash);
  const signed = ethers.Transaction.from({ ...tx, signature });
  if (signed.from?.toLowerCase() !== addresses.eth.toLowerCase()) throw new Error("Recovered ETH signer does not match the derived vault address.");
  if (!execute) return `ETH ready: would broadcast signed slot ${slotIndex} transaction`;
  let receipt = await evmProvider.getTransactionReceipt(signed.hash);
  if (!receipt) {
    try {
      await evmProvider.broadcastTransaction(signed.serialized);
    } catch (error) {
      // A prior run may already have broadcast this exact signed transaction.
      receipt = await evmProvider.getTransactionReceipt(signed.hash);
      if (!receipt) throw error;
    }
    if (!receipt) receipt = await evmProvider.waitForTransaction(signed.hash, 1, 600_000);
  }
  if (!receipt || receipt.status !== 1) throw new Error(`ETH transaction ${signed.hash} did not confirm successfully.`);
  await keeperAccount.callFunction({ contractId, methodName: "mark_done", args: { owner_id: ownerId, heir_index: slotIndex, tx_hash: signed.hash }, gas: 100_000_000_000_000n, attachedDeposit: 0n });
  return `ETH delivered: ${signed.hash} (block ${receipt.blockNumber})`;
}

async function fetchJson(url, options) {
  const response = await fetch(url, options);
  const text = await response.text();
  if (!response.ok) throw new Error(`Request failed (${response.status}): ${text.slice(0, 300)}`);
  try { return text ? JSON.parse(text) : null; } catch { return text; }
}

async function getBtcFeeRate() {
  const fees = await fetchJson(`${btcApi}/v1/fees/recommended`);
  const candidates = [fees.fastestFee, fees.halfHourFee, fees.economyFee]
    .filter((value) => Number.isFinite(Number(value)) && Number(value) > 0)
    .map((value) => BigInt(Math.ceil(Number(value))))
    .filter((value) => value <= BTC_MAX_FEE_RATE);
  if (!candidates.length) throw new Error("No current Bitcoin fee estimate fits the contract fee-rate cap.");
  return candidates[0];
}

async function signBtcTx(ownerId, status, addresses, utxo, feeRate) {
  const inputValue = BigInt(utxo.value);
  const outputs = calculateBtcOutputs(inputValue, feeRate, status.heirs);
  const tx = new bitcoin.Transaction();
  tx.version = 2;
  tx.addInput(Buffer.from(utxo.txid, "hex").reverse(), Number(utxo.vout), 0xfffffffd);
  for (const output of outputs) {
    const payment = bitcoin.payments.p2wpkh({ hash: Buffer.from(output.program, "hex"), network: bitcoin.networks.testnet });
    tx.addOutput(payment.output, Number(output.value));
  }
  const sighash = tx.hashForWitnessV0(0, Buffer.concat([Buffer.from("76a914", "hex"), Buffer.from(status.btc_sender_pubkey_hash, "hex"), Buffer.from("88ac", "hex")]), Number(inputValue), bitcoin.Transaction.SIGHASH_ALL);
  const witness = { owner_id: ownerId, txid: utxo.txid, vout: Number(utxo.vout), value_sats: inputValue.toString(), fee_rate_sat_vb: feeRate.toString() };
  return { tx, sighash: Buffer.from(sighash).toString("hex"), witness };
}

async function runBtc(ownerId, status, addresses) {
  if (status.btc_slot.status === "Done") return "BTC slot already Done";
  if (status.withdrawal_records?.some((record) => enumName(record.chain) === "Bitcoin" && ["Pending", "Signed"].includes(enumName(record.status)))) {
    return "BTC withdrawal is outstanding; skipped";
  }
  if (status.btc_slot.status === "Pending" || status.btc_slot.status === "Unsent") {
    const isRetry = status.btc_slot.status === "Pending";
    const utxos = await fetchJson(`${btcApi}/address/${addresses.btc}/utxo`);
    let candidate;
    let feeRate;
    if (isRetry) {
      const elapsedNs = BigInt(status.btc_slot.last_claim_timestamp_ns || "0");
      const currentNs = BigInt(status.current_timestamp_ns || "0");
      if (currentNs - elapsedNs < 60_000_000_000n) return "BTC MPC request is still within its 60 second retry window";
      candidate = utxos.find((u) => u.txid === status.btc_slot.pending_txid && Number(u.vout) === Number(status.btc_slot.pending_vout) && u.status?.confirmed);
      if (!candidate) return "BTC retry UTXO is no longer available from the public indexer; skipped";
      const suggested = await getBtcFeeRate();
      const previousRate = BigInt(status.btc_slot.pending_fee_rate_sat_vb);
      feeRate = suggested > previousRate ? suggested : previousRate + 1n;
      if (feeRate > BTC_MAX_FEE_RATE) return "BTC retry would exceed the contract fee-rate cap; skipped";
    } else {
      candidate = utxos.filter((u) => u.status?.confirmed).sort((a, b) => b.value - a.value)[0];
      feeRate = await getBtcFeeRate();
    }
    if (!candidate) return "No confirmed BTC UTXO; skipped";
    const prepared = await signBtcTx(ownerId, status, addresses, candidate, feeRate);
    if (!execute) return `BTC ready: would ${isRetry ? "retry" : "call claim_btc for"} ${candidate.txid}:${candidate.vout}`;
    await keeperAccount.callFunction({ contractId, methodName: "claim_btc", args: prepared.witness, gas: 300_000_000_000_000n, attachedDeposit: 0n });
    const updated = await waitForStatus(ownerId, (value) => ["Signed", "Unsent"].includes(value.btc_slot.status));
    if (!updated) return "BTC claim submitted; MPC callback has not settled yet, rerun later";
    if (updated.btc_slot.status !== "Signed") return "BTC MPC signing failed; contract restored the slot for a later attempt";
    return runBtc(ownerId, updated, addresses);
  }

  const slot = status.btc_slot;
  const txid = slot.last_txid;
  const vout = Number(slot.last_vout);
  if (!txid || !Number.isInteger(vout)) throw new Error("Signed BTC slot is missing its recorded UTXO.");
  const txInfo = await fetchJson(`${btcApi}/tx/${txid}`);
  const value = BigInt(txInfo.vout[vout].value);
  const feeRate = BigInt(slot.last_fee_rate_sat_vb);
  const prepared = await signBtcTx(ownerId, status, addresses, { txid, vout, value: value.toString() }, feeRate);
  if (prepared.sighash.toLowerCase() !== String(slot.signed_hash).replace(/^0x/i, "").toLowerCase()) throw new Error("BTC sighash does not match contract status; refusing to broadcast.");
  const signature = parseMpcSignature(slot.tx_hash);
  // Recover the signing public key from the BIP-143 digest, then check it is the vault's P2WPKH key.
  const r = BigInt(signature.r).toString(16).padStart(64, "0");
  const compact = Buffer.concat([Buffer.from(r, "hex"), Buffer.from(signature.s.slice(2), "hex")]);
  const recovered = ethers.SigningKey.recoverPublicKey(`0x${prepared.sighash}`, { ...signature });
  const compressedPubkey = Buffer.from(ethers.SigningKey.computePublicKey(recovered, true).slice(2), "hex");
  const pubkeyHash = Buffer.from(ripemd160(sha256(compressedPubkey))).toString("hex");
  if (pubkeyHash.toLowerCase() !== status.btc_sender_pubkey_hash.toLowerCase()) throw new Error("Recovered BTC signer does not match the vault witness program.");
  prepared.tx.setWitness(0, [bitcoin.script.signature.encode(compact, bitcoin.Transaction.SIGHASH_ALL), compressedPubkey]);
  const expectedTxid = prepared.tx.getId();
  const spend = await fetchJson(`${btcApi}/tx/${txid}/outspend/${vout}`);
  if (spend.spent && spend.txid !== expectedTxid) return `BTC input ${txid}:${vout} was spent by a different transaction; status reconciliation needed`;
  if (spend.spent && spend.txid === expectedTxid) {
    const known = await fetchJson(`${btcApi}/tx/${expectedTxid}/status`);
    if (!known.confirmed) return `BTC transaction ${expectedTxid} is already in the mempool; waiting for confirmation`;
    if (!execute) return `BTC ready: would report confirmed transaction ${expectedTxid}`;
    await keeperAccount.callFunction({ contractId, methodName: "mark_btc_done", args: { owner_id: ownerId, tx_hash: expectedTxid }, gas: 100_000_000_000_000n, attachedDeposit: 0n });
    return `BTC delivered: ${expectedTxid}`;
  }
  if (!execute) return `BTC ready: would broadcast signed transaction spending ${txid}:${vout}`;
  const txHex = prepared.tx.toHex();
  const broadcastTxid = await fetchJson(`${btcApi}/tx`, { method: "POST", headers: { "content-type": "text/plain" }, body: txHex });
  let confirmed = false;
  for (let attempt = 0; attempt < 60; attempt += 1) {
    const result = await fetchJson(`${btcApi}/tx/${broadcastTxid}/status`);
    if (result.confirmed) { confirmed = true; break; }
    await new Promise((resolve) => setTimeout(resolve, 10_000));
  }
  if (!confirmed) throw new Error(`BTC transaction ${broadcastTxid} was broadcast but is not confirmed yet; it was not marked Done.`);
  await keeperAccount.callFunction({ contractId, methodName: "mark_btc_done", args: { owner_id: ownerId, tx_hash: broadcastTxid }, gas: 100_000_000_000_000n, attachedDeposit: 0n });
  return `BTC delivered: ${broadcastTxid}`;
}

if (execute) {
  const privateKey = process.env.EMBER_KEEPER_PRIVATE_KEY?.trim();
  if (!privateKey) throw new Error("EMBER_KEEPER_PRIVATE_KEY is required when EMBER_KEEPER_EXECUTE=1.");
  keeperAccount = new Account(keeperId, nearProvider, KeyPairSigner.fromSecretKey(privateKey));
  await assertKeeperKeyScoped();
}

const owners = await readRegistry();
const selectedOwners = selectOwnerIds(owners, requestedOwnerId);
console.log(`Contract: ${contractId}`);
console.log(`Keeper: ${keeperId}`);
console.log(`Mode: ${execute ? "EXECUTE" : "DRY RUN"}`);
console.log(`Owner records found: ${owners.length}`);
console.log(`Owner selection: ${requestedOwnerId || "automatic scan"}`);
let processed = 0;
for (const ownerId of selectedOwners) {
  if (processed >= maxRecordsPerRun) break;
  const status = await view("status", { owner_id: ownerId });
  if (!status) {
    if (requestedOwnerId) throw new Error(`No status was returned for requested owner ${ownerId}.`);
    continue;
  }
  if (!Array.isArray(status.keepers) || !status.keepers.includes(keeperId)) {
    if (requestedOwnerId) throw new Error(`Keeper ${keeperId} is not authorized for requested owner ${ownerId}.`);
    continue;
  }
  const state = stateName(status.state);
  if (state !== "Claimable") {
    if (requestedOwnerId) throw new Error(`Requested owner ${ownerId} is ${state}, not Claimable.`);
    continue;
  }
  if (!hasUnfinishedClaimWork(status)) {
    console.log(`${ownerId}: all ETH and BTC claim slots are already Done; skipped`);
    if (requestedOwnerId) throw new Error(`Requested owner ${ownerId} has no unfinished claim slots.`);
    continue;
  }
  const addresses = await deriveAddresses(status);
  console.log(`${ownerId}: Claimable`);
  const ethResult = await runEth(ownerId, status, addresses);
  console.log(`  ETH: ${ethResult}`);
  // Re-read after ETH because its claim consumes the contract's one outstanding action per chain.
  const refreshed = await view("status", { owner_id: ownerId });
  const btcResult = await runBtc(ownerId, refreshed, addresses);
  console.log(`  BTC: ${btcResult}`);
  processed += 1;
}
console.log(`Eligible records processed: ${processed}`);
if (processed === 0 && !requestedOwnerId) console.log("No unfinished Claimable records were found for this keeper.");
