import { Account, JsonRpcProvider, KeyPairSigner, KeyPair } from 'near-api-js';
import { chainAdapters, contracts } from 'chainsig.js';

const NEAR_NETWORK_ID = 'testnet';
const VAULT_ACCOUNT_ID = process.env.NEAR_VAULT_ACCOUNT_ID || 'vault-heirvault01.testnet';
const VAULT_PRIVATE_KEY = process.env.NEAR_VAULT_PRIVATE_KEY;

if (!VAULT_PRIVATE_KEY) {
  console.error('Error: NEAR_VAULT_PRIVATE_KEY environment variable is required.');
  process.exit(1);
}

const BITCOIN_NETWORK = 'testnet'; // Bitcoin Testnet3
const MEMPOOL_API_URL = 'https://mempool.space/testnet/api';

async function main() {
  console.log('===============================================================');
  console.log('BITCOIN TESTNET PROOF USING CHAINSIG.JS & NEAR CHAIN SIGNATURES');
  console.log('===============================================================\n');

  // 1. Setup NEAR Account & Provider
  console.log('[1/5] Initializing NEAR account and provider...');
  const provider = new JsonRpcProvider({ url: 'https://rpc.testnet.near.org' });
  const keyPair = KeyPair.fromString(VAULT_PRIVATE_KEY);
  const signer = new KeyPairSigner(keyPair);
  const vaultAccount = new Account(VAULT_ACCOUNT_ID, provider, signer);
  console.log(`- Connected as: ${vaultAccount.accountId}`);

  // 2. Setup ChainSignatureContract and Bitcoin Adapter
  console.log('\n[2/5] Initializing ChainSignatureContract and Bitcoin Adapter...');
  const csContract = new contracts.ChainSignatureContract({
    networkId: NEAR_NETWORK_ID,
    contractId: 'v1.signer-prod.testnet'
  });

  const btcRpc = new chainAdapters.btc.Mempool(MEMPOOL_API_URL);
  const btc = new chainAdapters.btc.Bitcoin({
    network: BITCOIN_NETWORK,
    contract: csContract,
    btcRpcAdapter: btcRpc
  });

  // 3. Derive Sender & Recipient Addresses
  console.log('\n[3/5] Deriving Native SegWit (P2WPKH) Addresses from MPC...');
  const sender = await btc.deriveAddressAndPublicKey(VAULT_ACCOUNT_ID, 'bitcoin-1');
  console.log(`- Path 'bitcoin-1' (Sender Address):    ${sender.address}`);
  console.log(`  Public Key: ${sender.publicKey}`);

  const recipient = await btc.deriveAddressAndPublicKey(VAULT_ACCOUNT_ID, 'bitcoin-2');
  console.log(`- Path 'bitcoin-2' (Recipient Address): ${recipient.address}`);
  console.log(`  Public Key: ${recipient.publicKey}`);

  // 4. Check UTXOs and Balance on Bitcoin Testnet
  console.log('\n[4/5] Checking UTXOs for sender address on Mempool.space...');
  const utxos = await btcRpc.fetchUTXOs(sender.address);
  console.log(`- Found ${utxos.length} UTXO(s):`, JSON.stringify(utxos, null, 2));

  if (utxos.length === 0) {
    throw new Error(`No UTXO found for ${sender.address}`);
  }

  // 5. Send Native SegWit Transaction
  console.log('\n[5/5] Building, Signing, and Broadcasting Bitcoin Transaction...');
  const utxo = utxos[0];
  const totalValue = utxo.value; // satoshis
  console.log(`- Using UTXO: ${utxo.txid}:${utxo.vout} (${totalValue} satoshis)`);

  const feeRate = await btcRpc.fetchFeeRate(6); // sat/vB
  console.log(`- Recommended fee rate: ${feeRate} sat/vB`);

  // Native SegWit 1-input 1-output transaction is ~110 vB.
  // We use generous 140 vB * feeRate (minimum 2 sat/vB)
  const feeRateEffective = Math.max(feeRate || 1, 2);
  const estimatedFee = Math.ceil(140 * feeRateEffective);
  const sendAmountSatoshis = totalValue - estimatedFee;

  console.log(`- Estimated fee: ${estimatedFee} satoshis`);
  console.log(`- Send amount:   ${sendAmountSatoshis} satoshis (${sendAmountSatoshis / 1e8} BTC)`);

  if (sendAmountSatoshis <= 546) {
    throw new Error(`Send amount (${sendAmountSatoshis}) below Bitcoin dust limit (546 satoshis)`);
  }

  const txRequest = {
    from: sender.address,
    to: recipient.address,
    publicKey: sender.publicKey,
    inputs: [
      {
        txid: utxo.txid,
        vout: utxo.vout,
        value: utxo.value
      }
    ],
    outputs: [
      {
        address: recipient.address,
        value: sendAmountSatoshis
      }
    ]
  };

  console.log('\nPreparing PSBT for signing...');
  const { transaction, hashesToSign } = await btc.prepareTransactionForSigning(txRequest);
  console.log(`- Generated ${hashesToSign.length} hash(es) to sign:`);
  hashesToSign.forEach((h, idx) => {
    console.log(`  [Input #${idx}] BIP-143 Sighash: ${Buffer.from(h).toString('hex')}`);
  });

  console.log('\nRequesting MPC signature(s) from v1.signer-prod.testnet...');
  const rsvSignatures = await csContract.sign({
    payloads: hashesToSign,
    path: 'bitcoin-1',
    keyType: 'Ecdsa',
    signerAccount: vaultAccount
  });
  console.log('- Received MPC signature(s):', JSON.stringify(rsvSignatures, null, 2));

  console.log('\nFinalizing transaction signing and assembling witness...');
  const signedTxHex = btc.finalizeTransactionSigning({
    transaction,
    rsvSignatures
  });
  console.log(`- Signed Raw Tx Hex:\n${signedTxHex}\n`);

  console.log('Broadcasting transaction to Bitcoin Testnet via Mempool.space...');
  const broadcastResult = await btc.broadcastTx(signedTxHex);
  console.log(`\n🎉 TRANSACTION BROADCAST SUCCESSFUL!`);
  console.log(`- Txid: ${broadcastResult.hash}`);
  console.log(`- Mempool Explorer: https://mempool.space/testnet/tx/${broadcastResult.hash}`);

  return {
    status: 'SUCCESS',
    txid: broadcastResult.hash,
    explorerLink: `https://mempool.space/testnet/tx/${broadcastResult.hash}`
  };
}

main().catch(err => {
  console.error('\n❌ EXECUTION FAILED:');
  console.error(err);
  process.exit(1);
});
