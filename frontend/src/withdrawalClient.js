import { JsonRpcProvider, Transaction } from 'ethers';
import * as bitcoin from 'bitcoinjs-lib';
import { secp256k1 } from '@noble/curves/secp256k1';
import { VAULT_ACCOUNT_ID } from './config.js';
import { callWithdrawalMethod } from './vaultClient.js';

const ETH_RPC = 'https://ethereum-sepolia-rpc.publicnode.com';
const BTC_API = 'https://blockstream.info/testnet/api';
const ETH_CHAIN_ID = 11155111;
const BTC_MAX_FEE_RATE = 10_000;
const provider = new JsonRpcProvider(ETH_RPC);

function asNumber(value) {
  const n = Number(value || 0);
  if (!Number.isSafeInteger(n)) throw new Error('The contract returned a number this browser cannot safely process.');
  return n;
}

function variant(value) {
  return typeof value === 'string' ? value : Object.keys(value || {})[0] || 'Unknown';
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, options);
  if (!response.ok) throw new Error(`Public testnet service returned ${response.status}.`);
  return response.json();
}

function parseMpcSignature(raw) {
  let value;
  try { value = JSON.parse(raw); } catch { throw new Error('The contract has no readable MPC signature for this withdrawal.'); }
  if (Array.isArray(value)) value = value[0];
  const r = value?.big_r?.affine_point?.slice(2) || value?.r?.replace(/^0x/, '');
  const s = value?.s?.scalar || (typeof value?.s === 'string' ? value.s.replace(/^0x/, '') : '');
  const recovery = Number(value?.recovery_id ?? value?.recoveryId);
  if (!/^[\da-f]{64}$/i.test(r || '') || !/^[\da-f]{64}$/i.test(s || '') || ![0, 1, 2, 3].includes(recovery)) {
    throw new Error('The MPC signature has an unsupported format.');
  }
  return { r: `0x${r}`, s: `0x${s}`, recovery, compact: `${r}${s}` };
}

function secretKey(ownerId, chain, nonce) {
  return `ember-withdrawal:${ownerId}:${chain}:${nonce}`;
}

export async function getEthWithdrawalPreview(address, contractStatus) {
  const network = await provider.getNetwork();
  if (Number(network.chainId) !== ETH_CHAIN_ID) throw new Error('The configured RPC is not Sepolia.');
  const [balance, nonce, feeData] = await Promise.all([
    provider.getBalance(address),
    provider.getTransactionCount(address, 'pending'),
    provider.getFeeData(),
  ]);
  const gasPrice = feeData.gasPrice;
  if (!gasPrice) throw new Error('The Sepolia RPC did not return a gas price.');
  const fee = BigInt(contractStatus.gas_limit) * gasPrice;
  const feeCap = 2_000_000_000_000_000n;
  if (fee > feeCap) throw new Error('Current estimated fee exceeds the contract’s 0.002 ETH cap. Try again later.');
  if (fee >= balance) throw new Error('This balance is too small to cover the estimated transaction fee.');
  if (fee * 100n > balance * 5n) throw new Error('Estimated fee exceeds the contract’s 5% balance limit.');
  return { balance: balance.toString(), nonce, gasPrice: gasPrice.toString(), estimatedFee: fee.toString(), amount: (balance - fee).toString() };
}

export async function getBtcWithdrawalPreview(address) {
  const [utxos, fees] = await Promise.all([
    fetchJson(`${BTC_API}/address/${encodeURIComponent(address)}/utxo`),
    fetchJson(`${BTC_API}/fee-estimates`),
  ]);
  const confirmed = utxos.filter((utxo) => utxo.status?.confirmed).sort((a, b) => b.value - a.value);
  if (!confirmed.length) throw new Error('No confirmed Bitcoin Testnet UTXO is available to withdraw.');
  const feeRate = Math.max(1, Math.ceil(Number(fees['6'] || fees['3'] || 2)));
  if (!Number.isSafeInteger(feeRate) || feeRate > BTC_MAX_FEE_RATE) throw new Error('The current Bitcoin fee estimate is outside the contract limit.');
  const input = confirmed[0];
  const fee = 110n * BigInt(feeRate);
  if (fee * 100n > BigInt(input.value) * 5n) throw new Error('Estimated Bitcoin fee exceeds the contract’s 5% input limit.');
  if (BigInt(input.value) - fee < 294n) throw new Error('This UTXO would leave less than the Bitcoin dust limit after fees.');
  return { txid: input.txid, vout: input.vout, valueSats: input.value, feeRate, estimatedFee: fee.toString(), amount: (BigInt(input.value) - fee).toString() };
}

export async function requestEthWithdrawal(wallet, ownerId, status, address, contractId = VAULT_ACCOUNT_ID, prepared = null) {
  if (status.eth_outstanding_nonce != null) throw new Error('An Ethereum action is already outstanding. Resolve it before starting another.');
  const fresh = await getEthWithdrawalPreview(address, status);
  if (prepared && (fresh.balance !== prepared.balance || fresh.nonce !== prepared.nonce || fresh.gasPrice !== prepared.gasPrice)) {
    throw new Error('The Ethereum balance, nonce, or gas price changed after the preview. Review a fresh preview before continuing.');
  }
  const preview = fresh;
  await callWithdrawalMethod(wallet, ownerId, contractId, 'withdraw_eth', {
    balance_wei: preview.balance,
    nonce: String(preview.nonce),
    gas_price_wei: preview.gasPrice,
  });
  return preview;
}

export async function requestBtcWithdrawal(wallet, ownerId, status, address, contractId = VAULT_ACCOUNT_ID, prepared = null) {
  const pending = (status.withdrawal_records || []).some((record) => variant(record.chain) === 'Bitcoin' && ['Pending', 'Signed'].includes(variant(record.status)));
  if (pending || ['Pending', 'Signed'].includes(variant(status.btc_slot?.status))) {
    throw new Error('A Bitcoin action is already outstanding. Resolve it before starting another.');
  }
  const fresh = await getBtcWithdrawalPreview(address);
  if (prepared && (fresh.txid !== prepared.txid || fresh.vout !== prepared.vout || fresh.valueSats !== prepared.valueSats || fresh.feeRate !== prepared.feeRate)) {
    throw new Error('The available Bitcoin UTXO or fee estimate changed after the preview. Review a fresh preview before continuing.');
  }
  const preview = fresh;
  await callWithdrawalMethod(wallet, ownerId, contractId, 'withdraw_btc', {
    txid: preview.txid,
    vout: preview.vout,
    value_sats: String(preview.valueSats),
    fee_rate_sat_vb: String(preview.feeRate),
  });
  return preview;
}

export async function broadcastEthWithdrawal(wallet, ownerId, record, status, depositAddresses, contractId = VAULT_ACCOUNT_ID) {
  if (variant(record.status) !== 'Signed' || variant(record.chain) !== 'Ethereum') throw new Error('This Ethereum withdrawal is not ready to broadcast.');
  if (String(status.eth_outstanding_nonce) !== String(record.nonce)) throw new Error('This Ethereum withdrawal no longer holds the contract’s outstanding nonce. Refresh the plan before broadcasting.');
  const saved = localStorage.getItem(secretKey(ownerId, 'Ethereum', record.nonce));
  if (!saved) throw new Error('This withdrawal’s transaction details are not in this browser session. Start from its pending record or wait for the outstanding action to clear.');
  const { gasPrice } = JSON.parse(saved);
  const to = record.destination;
  if (!to || !record.nonce) throw new Error('The active withdrawal destination or nonce is missing.');
  const rawSig = parseMpcSignature(record.tx_hash);
  const unsigned = { type: 1, chainId: ETH_CHAIN_ID, nonce: asNumber(record.nonce), gasPrice: BigInt(gasPrice),
    gasLimit: BigInt(status.gas_limit), to, value: BigInt(record.amount), data: '0x' };
  const signed = Transaction.from({ ...unsigned, signature: { r: rawSig.r, s: rawSig.s, yParity: rawSig.recovery & 1 } });
  if (signed.from?.toLowerCase() !== depositAddresses?.ethereum?.address?.toLowerCase() || signed.to?.toLowerCase() !== to.toLowerCase()) {
    throw new Error('The signature does not match this record’s NEAR-derived Ethereum address and registered destination.');
  }
  const broadcast = await provider.broadcastTransaction(signed.serialized);
  const receipt = await broadcast.wait(1);
  if (!receipt || receipt.status !== 1) throw new Error('The Sepolia transaction did not confirm successfully.');
  await callWithdrawalMethod(wallet, ownerId, contractId, 'mark_withdrawal_done', {
    owner_id: ownerId,
    chain: 'Ethereum',
    tx_hash: broadcast.hash,
  });
  localStorage.removeItem(secretKey(ownerId, 'Ethereum', record.nonce));
  return broadcast.hash;
}

export async function broadcastBtcWithdrawal(wallet, ownerId, record, status, depositAddresses, contractId = VAULT_ACCOUNT_ID) {
  if (variant(record.status) !== 'Signed' || variant(record.chain) !== 'Bitcoin') throw new Error('This Bitcoin withdrawal is not ready to broadcast.');
  const tx = new bitcoin.Transaction();
  tx.version = 2;
  tx.addInput(Buffer.from(record.txid, 'hex').reverse(), Number(record.vout), 0xfffffffd);
  const witnessProgram = Buffer.from(record.destination, 'hex');
  const outputScript = bitcoin.script.compile([bitcoin.opcodes.OP_0, witnessProgram]);
  const amount = Number(record.amount);
  if (!Number.isSafeInteger(amount) || amount <= 0) throw new Error('The recorded Bitcoin amount is invalid.');
  tx.addOutput(outputScript, amount);
  const senderProgram = Buffer.from(status.btc_sender_pubkey_hash, 'hex');
  const scriptCode = bitcoin.payments.p2pkh({ hash: senderProgram, network: bitcoin.networks.testnet }).output;
  const inputValue = Number(record.value_sats);
  if (!Number.isSafeInteger(inputValue) || inputValue <= 0) throw new Error('The recorded Bitcoin input value is invalid.');
  if (BigInt(record.amount) !== BigInt(inputValue) - 110n * BigInt(record.fee_rate_sat_vb)) {
    throw new Error('The recorded Bitcoin amount does not match the contract fee calculation.');
  }
  const sighash = tx.hashForWitnessV0(0, scriptCode, inputValue, bitcoin.Transaction.SIGHASH_ALL);
  const uncompressedKey = Buffer.from(depositAddresses?.bitcoin?.publicKey || '', 'hex');
  if (uncompressedKey.length !== 65 || uncompressedKey[0] !== 4) throw new Error('The derived Bitcoin public key is unavailable.');
  const compressedKey = Buffer.concat([Buffer.from([uncompressedKey[64] & 1 ? 3 : 2]), uncompressedKey.subarray(1, 33)]);
  if (!bitcoin.crypto.hash160(compressedKey).equals(senderProgram)) throw new Error('The derived Bitcoin key does not match the contract’s deposit address.');
  const signature = parseMpcSignature(record.tx_hash);
  const compact = Buffer.from(signature.compact, 'hex');
  if (!secp256k1.verify(compact, sighash, compressedKey)) throw new Error('The MPC signature does not match the recorded Bitcoin withdrawal transaction.');
  const n = secp256k1.Point.Fn.ORDER;
  const r = BigInt(`0x${signature.compact.slice(0, 64)}`);
  let s = BigInt(`0x${signature.compact.slice(64)}`);
  if (s > n / 2n) s = n - s;
  const derInteger = (value) => {
    let bytes = Buffer.from(value.toString(16).padStart(64, '0'), 'hex');
    while (bytes.length > 1 && bytes[0] === 0 && !(bytes[1] & 0x80)) bytes = bytes.subarray(1);
    if (bytes[0] & 0x80) bytes = Buffer.concat([Buffer.from([0]), bytes]);
    return Buffer.concat([Buffer.from([2, bytes.length]), bytes]);
  };
  const derR = derInteger(r);
  const derS = derInteger(s);
  const der = Buffer.concat([Buffer.from([0x30, derR.length + derS.length]), derR, derS]);
  tx.setWitness(0, [Buffer.concat([der, Buffer.from([bitcoin.Transaction.SIGHASH_ALL])]), compressedKey]);
  const raw = tx.toHex();
  const response = await fetch(`${BTC_API}/tx`, { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: raw });
  if (!response.ok) throw new Error(`Bitcoin Testnet broadcast failed (${response.status}): ${(await response.text()).slice(0, 300)}`);
  const txid = (await response.text()).trim();
  if (!/^[\da-f]{64}$/i.test(txid)) throw new Error('Bitcoin Testnet returned an invalid transaction ID.');
  await callWithdrawalMethod(wallet, ownerId, contractId, 'mark_withdrawal_done', {
    owner_id: ownerId,
    chain: 'Bitcoin',
    tx_hash: txid,
  });
  return txid;
}

export function storeEthWithdrawalSession(ownerId, record, gasPrice) {
  localStorage.setItem(secretKey(ownerId, 'Ethereum', record.nonce), JSON.stringify({ gasPrice }));
}
