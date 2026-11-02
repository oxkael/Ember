import crypto from 'crypto';
import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';

const VAULT_ID = 'vault-heirvault01.testnet';
const HEIR1_ID = 'heir1-heirvault01.testnet';
const HEIR2_ID = 'heir2-heirvault01.testnet';
const RPC_URL = 'https://rpc.testnet.near.org';
const BLOCKSTREAM_API_URL = 'https://blockstream.info/testnet/api';

const BTC1_ADDRESS = 'tb1qwlkej62grq3c2juvs595t94pvqctkdyq6qyygg';
const BTC2_ADDRESS = 'tb1q3y2vrtuvvjkttcqgpl2duxqaxrjy26k54r064j';

const SENDER_PUBKEY = '0234998c6a4ce7a6f85831292649ca66c02d30c530be7d00eb95c9d917ace5c7e5';
const SENDER_PKH = '77ed9969481823854b8c850b4596a16030bb3480';

function sha256(data) {
  return crypto.createHash('sha256').update(data).digest();
}

function doubleSha256(data) {
  return sha256(sha256(data));
}

function computeBip143Sighash(txidHex, vout, senderPkhHex, valueSats, sequence, outputs, locktime = 0) {
  const versionBuf = Buffer.alloc(4);
  versionBuf.writeUInt32LE(2);

  const txidBuf = Buffer.from(txidHex, 'hex').reverse();
  const voutBuf = Buffer.alloc(4);
  voutBuf.writeUInt32LE(vout);

  const prevouts = Buffer.concat([txidBuf, voutBuf]);
  const hashPrevouts = doubleSha256(prevouts);

  const seqBuf = Buffer.alloc(4);
  seqBuf.writeUInt32LE(sequence);
  const hashSequence = doubleSha256(seqBuf);

  const outpoint = Buffer.concat([txidBuf, voutBuf]);

  const pkhBuf = Buffer.from(senderPkhHex, 'hex');
  const scriptCode = Buffer.concat([
    Buffer.from([0x19, 0x76, 0xa9, 0x14]),
    pkhBuf,
    Buffer.from([0x88, 0xac])
  ]);

  const valueBuf = Buffer.alloc(8);
  valueBuf.writeBigUInt64LE(BigInt(valueSats));

  let outputsBuf = Buffer.alloc(0);
  for (const out of outputs) {
    const valBuf = Buffer.alloc(8);
    valBuf.writeBigUInt64LE(BigInt(out.valueSats));
    const progBuf = Buffer.from(out.witnessProgramHex, 'hex');
    const spk = Buffer.concat([Buffer.from([0x16, 0x00, 0x14]), progBuf]);
    outputsBuf = Buffer.concat([outputsBuf, valBuf, spk]);
  }
  const hashOutputs = doubleSha256(outputsBuf);

  const locktimeBuf = Buffer.alloc(4);
  locktimeBuf.writeUInt32LE(locktime);

  const sighashTypeBuf = Buffer.alloc(4);
  sighashTypeBuf.writeUInt32LE(1); // SIGHASH_ALL

  const preimage = Buffer.concat([
    versionBuf,
    hashPrevouts,
    hashSequence,
    outpoint,
    scriptCode,
    valueBuf,
    seqBuf,
    hashOutputs,
    locktimeBuf,
    sighashTypeBuf
  ]);

  return doubleSha256(preimage);
}

function toDer(rHex, sHex) {
  let r = Buffer.from(rHex, 'hex');
  let s = Buffer.from(sHex, 'hex');
  const N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141n;
  let sBig = BigInt('0x' + sHex);
  if (sBig > N / 2n) {
    sBig = N - sBig;
    s = Buffer.from(sBig.toString(16).padStart(64, '0'), 'hex');
  }
  if (r[0] & 0x80) r = Buffer.concat([Buffer.from([0x00]), r]);
  if (s[0] & 0x80) s = Buffer.concat([Buffer.from([0x00]), s]);
  return Buffer.concat([
    Buffer.from([0x30, r.length + s.length + 4, 0x02, r.length]),
    r,
    Buffer.from([0x02, s.length]),
    s
  ]);
}

function buildSignedSegwitTx({ txidHex, vout, sequence, outputs, derSig, pubkeyHex, locktime = 0 }) {
  const version = Buffer.alloc(4);
  version.writeUInt32LE(2);

  const marker = Buffer.from([0x00, 0x01]); // Segwit marker & flag

  const inputCount = Buffer.from([0x01]);
  const txidBuf = Buffer.from(txidHex, 'hex').reverse();
  const voutBuf = Buffer.alloc(4);
  voutBuf.writeUInt32LE(vout);
  const scriptSig = Buffer.from([0x00]); // empty scriptSig for Segwit
  const seqBuf = Buffer.alloc(4);
  seqBuf.writeUInt32LE(sequence);

  const inputBuf = Buffer.concat([txidBuf, voutBuf, scriptSig, seqBuf]);

  const outputCount = Buffer.from([outputs.length]);
  let outputsBuf = Buffer.alloc(0);
  for (const out of outputs) {
    const valBuf = Buffer.alloc(8);
    valBuf.writeBigUInt64LE(BigInt(out.valueSats));
    const progBuf = Buffer.from(out.witnessProgramHex, 'hex');
    const spk = Buffer.concat([Buffer.from([0x16, 0x00, 0x14]), progBuf]);
    outputsBuf = Buffer.concat([outputsBuf, valBuf, spk]);
  }

  const sigWithHashType = Buffer.concat([derSig, Buffer.from([0x01])]); // SIGHASH_ALL = 0x01
  const pubkeyBuf = Buffer.from(pubkeyHex, 'hex');

  const witnessStackCount = Buffer.from([0x02]); // 2 items: signature, public key
  const sigLen = Buffer.from([sigWithHashType.length]);
  const pubkeyLen = Buffer.from([pubkeyBuf.length]);

  const witnessBuf = Buffer.concat([
    witnessStackCount,
    sigLen,
    sigWithHashType,
    pubkeyLen,
    pubkeyBuf
  ]);

  const locktimeBuf = Buffer.alloc(4);
  locktimeBuf.writeUInt32LE(locktime);

  return Buffer.concat([
    version,
    marker,
    inputCount,
    inputBuf,
    outputCount,
    outputsBuf,
    witnessBuf,
    locktimeBuf
  ]);
}

async function getContractStatus() {
  const res = await fetch(RPC_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 'dontcare',
      method: 'query',
      params: {
        request_type: 'call_function',
        finality: 'final',
        account_id: VAULT_ID,
        method_name: 'status',
        args_base64: Buffer.from('{}').toString('base64')
      }
    })
  });
  const data = await res.json();
  if (data.error) throw new Error(JSON.stringify(data.error));
  const rawStr = Buffer.from(data.result.result).toString('utf8');
  return JSON.parse(rawStr);
}

async function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}

async function fetchUtxos(addr) {
  const res = await fetch(`${BLOCKSTREAM_API_URL}/address/${addr}/utxo`);
  return await res.json();
}

async function fetchAddressInfo(addr) {
  const res = await fetch(`${BLOCKSTREAM_API_URL}/address/${addr}`);
  return await res.json();
}

async function main() {
  console.log('===============================================================');
  console.log('STEP 4: BITCOIN UTXO AUDIT & INHERITANCE CLAIM PIPELINE');
  console.log('===============================================================\n');

  // 1. List UTXOs at bitcoin-1 and bitcoin-2 with confirmation status
  console.log('[1/7] Auditing UTXOs at bitcoin-1 and bitcoin-2...');
  const utxos1 = await fetchUtxos(BTC1_ADDRESS);
  const utxos2 = await fetchUtxos(BTC2_ADDRESS);

  console.log(`\n--- UTXOs at bitcoin-1 (${BTC1_ADDRESS}) ---`);
  console.log(`Total count: ${utxos1.length}`);
  for (const u of utxos1) {
    console.log(`- ${u.txid}:${u.vout} | Value: ${u.value} sats | Confirmed: ${u.status.confirmed} (block: ${u.status.block_height || 'mempool'})`);
  }

  console.log(`\n--- UTXOs at bitcoin-2 (${BTC2_ADDRESS}) ---`);
  console.log(`Total count: ${utxos2.length}`);
  for (const u of utxos2) {
    console.log(`- ${u.txid}:${u.vout} | Value: ${u.value} sats | Confirmed: ${u.status.confirmed} (block: ${u.status.block_height || 'mempool'})`);
  }

  const confirmedUtxos1 = utxos1.filter(u => u.status && u.status.confirmed);
  if (confirmedUtxos1.length !== 1) {
    console.error(`\n❌ STOPPING: bitcoin-1 does not have exactly one confirmed UTXO! Found: ${confirmedUtxos1.length} confirmed UTXO(s).`);
    process.exit(1);
  }
  console.log(`\n✅ Exact single confirmed UTXO verified at bitcoin-1!`);

  const utxo = confirmedUtxos1[0];

  // 2. Verify vault contract state
  console.log('\n[2/7] Checking vault contract status on NEAR testnet...');
  let status = await getContractStatus();
  console.log(`- Vault state: ${status.state}`);
  console.log(`- BTC slot initial status: ${status.btc_slot.status}`);

  if (status.state !== 'Claimable') {
    throw new Error(`Vault is not Claimable yet! Current state: ${status.state}`);
  }

  // 3. Call claim_btc as heir1 at fee rate 2
  console.log('\n[3/7] Calling claim_btc as heir1-heirvault01.testnet (fee rate: 2 sat/vB)...');
  const feeRateSatVb = 2;
  const claimArgs = {
    txid: utxo.txid,
    vout: utxo.vout,
    value_sats: utxo.value.toString(),
    fee_rate_sat_vb: feeRateSatVb.toString()
  };

  const argsPath = path.resolve('claim-btc-args.json');
  fs.writeFileSync(argsPath, JSON.stringify(claimArgs, null, 2));

  const claimCmd = `near contract call-function as-transaction ${VAULT_ID} claim_btc file-args "${argsPath}" prepaid-gas "300.0 Tgas" attached-deposit "0.05 NEAR" sign-as ${HEIR1_ID} network-config testnet sign-with-keychain send`;
  console.log(`Executing: ${claimCmd}`);
  execSync(claimCmd, { encoding: 'utf8' });
  console.log('claim_btc call submitted successfully!');

  // 4. Poll contract status until btc_slot is Signed
  console.log('\n[4/7] Polling contract status for MPC signature...');
  let btcSlot;
  for (let attempt = 1; attempt <= 30; attempt++) {
    await sleep(4000);
    status = await getContractStatus();
    btcSlot = status.btc_slot;
    console.log(`- Attempt ${attempt}: btc_slot.status = ${btcSlot.status}`);
    if (btcSlot.status === 'Signed') {
      break;
    }
  }

  if (btcSlot.status !== 'Signed') {
    throw new Error(`BTC slot did not reach Signed state (current: ${btcSlot.status})`);
  }

  console.log('\n✅ MPC Signature received and committed to contract:');
  console.log(`- Contract Signed Hash: ${btcSlot.signed_hash}`);
  console.log(`- Stored MPC Sig Data: ${btcSlot.tx_hash}`);

  // 5. Rebuild transaction and verify sighash locally
  console.log('\n[5/7] Rebuilding transaction and verifying sighash locally...');
  const heirsConfig = status.heirs.map(h => h.heir);
  const numHeirs = heirsConfig.length;
  const vbytes = 11 + 68 + 31 * numHeirs;
  const feeSats = vbytes * feeRateSatVb;
  const poolSats = utxo.value - feeSats;

  console.log(`- Input Value: ${utxo.value} sats`);
  console.log(`- Vbytes: ${vbytes}, Fee Rate: ${feeRateSatVb} sat/vB, Total Fee: ${feeSats} sats`);
  console.log(`- Distributable Pool: ${poolSats} sats`);

  let allocated = 0;
  const outputs = [];
  for (let i = 0; i < numHeirs; i++) {
    const heir = heirsConfig[i];
    let outputVal;
    if (i === numHeirs - 1) {
      outputVal = poolSats - allocated;
    } else {
      outputVal = Math.floor((poolSats * heir.share_basis_points) / 10000);
    }
    allocated += outputVal;
    outputs.push({
      valueSats: outputVal,
      witnessProgramHex: heir.bitcoin_witness_program,
      nearAccountId: heir.near_account_id
    });
    console.log(`  Output #${i}: ${outputVal} sats to witness program ${heir.bitcoin_witness_program} (${heir.near_account_id})`);
  }

  const sequence = 0xfffffffd;
  const localSighashBuf = computeBip143Sighash(
    utxo.txid,
    utxo.vout,
    SENDER_PKH,
    utxo.value,
    sequence,
    outputs,
    0
  );
  const localSighashHex = localSighashBuf.toString('hex').toLowerCase();
  const contractSighashHex = btcSlot.signed_hash.toLowerCase();

  console.log(`- Locally Built BIP-143 Sighash: ${localSighashHex}`);
  console.log(`- Contract Stored Sighash:        ${contractSighashHex}`);

  if (localSighashHex !== contractSighashHex) {
    throw new Error(`CRITICAL SIGHASH MISMATCH: Local ${localSighashHex} !== Contract ${contractSighashHex}`);
  }
  console.log('✅ SIGHASH MATCH CONFIRMED (100% cryptographic parity)');

  // Assemble signed transaction
  let sigObj = JSON.parse(btcSlot.tx_hash);
  if (Array.isArray(sigObj)) sigObj = sigObj[0];
  const rHex = sigObj.big_r?.affine_point
    ? sigObj.big_r.affine_point.substring(2)
    : (typeof sigObj.r === 'string' ? sigObj.r : '');
  const sHex = sigObj.s?.scalar
    ? sigObj.s.scalar
    : (typeof sigObj.s === 'string' ? sigObj.s : '');

  const derSig = toDer(rHex, sHex);
  const rawTxBuf = buildSignedSegwitTx({
    txidHex: utxo.txid,
    vout: utxo.vout,
    sequence,
    outputs,
    derSig,
    pubkeyHex: SENDER_PUBKEY,
    locktime: 0
  });

  const rawTxHex = rawTxBuf.toString('hex');
  console.log(`\n- Signed Raw Tx Hex (${rawTxBuf.length} bytes):\n${rawTxHex}\n`);

  // 6. Broadcast to Bitcoin Testnet
  console.log('[6/7] Broadcasting transaction to Bitcoin Testnet via Blockstream API...');
  const bcRes = await fetch(`${BLOCKSTREAM_API_URL}/tx`, {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: rawTxHex
  });

  if (!bcRes.ok) {
    const errText = await bcRes.text();
    throw new Error(`Broadcast failed (${bcRes.status}): ${errText}`);
  }
  const broadcastTxid = await bcRes.text();
  console.log(`🚀 BROADCAST SUCCESSFUL! Txid: ${broadcastTxid}`);
  const txExplorerUrl = `https://blockstream.info/testnet/tx/${broadcastTxid}`;
  console.log(`Explorer URL: ${txExplorerUrl}`);

  // Wait for at least one confirmation
  console.log('\nWaiting for at least 1 confirmation on Bitcoin Testnet...');
  let confirmedBlockHeight = null;
  while (true) {
    await sleep(10000);
    const txStatusRes = await fetch(`${BLOCKSTREAM_API_URL}/tx/${broadcastTxid}/status`);
    if (txStatusRes.ok) {
      const txStatus = await txStatusRes.json();
      console.log(`  Confirmation status: ${txStatus.confirmed ? 'CONFIRMED' : 'pending in mempool'}`);
      if (txStatus.confirmed) {
        confirmedBlockHeight = txStatus.block_height;
        console.log(`✅ Transaction CONFIRMED in block #${confirmedBlockHeight}!`);
        break;
      }
    }
  }

  // 7. Have both heirs call mark_btc_done
  console.log('\n[7/7] Having both heirs call mark_btc_done...');

  console.log(`- Calling mark_btc_done as ${HEIR1_ID}...`);
  const markCmd1 = `near contract call-function as-transaction ${VAULT_ID} mark_btc_done file-args "C:\\Users\\user\\Desktop\\Heir\\empty.json" prepaid-gas "100.0 Tgas" attached-deposit "0 NEAR" sign-as ${HEIR1_ID} network-config testnet sign-with-keychain send`;
  execSync(markCmd1, { encoding: 'utf8' });
  console.log(`  mark_btc_done called by ${HEIR1_ID}`);

  status = await getContractStatus();
  console.log(`  Confirmations: [${status.btc_slot.confirmations.join(', ')}] | Status: ${status.btc_slot.status}`);

  console.log(`- Calling mark_btc_done as ${HEIR2_ID}...`);
  const markCmd2 = `near contract call-function as-transaction ${VAULT_ID} mark_btc_done file-args "C:\\Users\\user\\Desktop\\Heir\\empty.json" prepaid-gas "100.0 Tgas" attached-deposit "0 NEAR" sign-as ${HEIR2_ID} network-config testnet sign-with-keychain send`;
  execSync(markCmd2, { encoding: 'utf8' });
  console.log(`  mark_btc_done called by ${HEIR2_ID}`);

  status = await getContractStatus();
  console.log(`  Confirmations: [${status.btc_slot.confirmations.join(', ')}] | Status: ${status.btc_slot.status}`);

  // 8. Fetch wallet balances from explorer
  console.log('\nFetching final heir wallet balances from Blockstream Testnet explorer API...');
  const heir1Addr = 'tb1q9mzed7tf3690p62uzpa3482kvhr2nyg30fhklm';
  const heir2Addr = 'tb1qzd332jveacrju9hcpqamyj3v6h2zpuxhjuf23p';

  const [h1Info, h2Info] = await Promise.all([
    fetchAddressInfo(heir1Addr),
    fetchAddressInfo(heir2Addr)
  ]);

  const h1ConfirmedSats = (h1Info.chain_stats.funded_txo_sum - h1Info.chain_stats.spent_txo_sum);
  const h1MempoolSats = (h1Info.mempool_stats.funded_txo_sum - h1Info.mempool_stats.spent_txo_sum);
  const h2ConfirmedSats = (h2Info.chain_stats.funded_txo_sum - h2Info.chain_stats.spent_txo_sum);
  const h2MempoolSats = (h2Info.mempool_stats.funded_txo_sum - h2Info.mempool_stats.spent_txo_sum);

  console.log('\n===============================================================');
  console.log('=== BITCOIN CLAIM WORKFLOW SUMMARY ===');
  console.log('===============================================================');
  console.log(`Broadcast Transaction ID: ${broadcastTxid}`);
  console.log(`Transaction Explorer URL: ${txExplorerUrl}`);
  console.log(`Confirmed in Block Height: #${confirmedBlockHeight}`);
  console.log(`\nOutputs:`);
  for (let i = 0; i < outputs.length; i++) {
    const o = outputs[i];
    console.log(`  Output #${i}: ${o.valueSats} sats -> Program: ${o.witnessProgramHex} (Beneficiary: ${o.nearAccountId})`);
  }
  console.log(`\nHeir 1 Address: ${heir1Addr}`);
  console.log(`  Explorer URL: https://blockstream.info/testnet/address/${heir1Addr}`);
  console.log(`  Confirmed Balance: ${h1ConfirmedSats} sats (${h1ConfirmedSats / 1e8} BTC) | Mempool: ${h1MempoolSats} sats`);
  console.log(`Heir 2 Address: ${heir2Addr}`);
  console.log(`  Explorer URL: https://blockstream.info/testnet/address/${heir2Addr}`);
  console.log(`  Confirmed Balance: ${h2ConfirmedSats} sats (${h2ConfirmedSats / 1e8} BTC) | Mempool: ${h2MempoolSats} sats`);
  console.log(`\nBTC Slot Final State in Contract: ${status.btc_slot.status}`);
  console.log(`BTC Slot Confirmations: [${status.btc_slot.confirmations.join(', ')}]`);
}

main().catch(err => {
  console.error('\n❌ Workflow failed:', err);
  process.exit(1);
});
