import { Account, JsonRpcProvider, KeyPairSigner, KeyPair } from 'near-api-js';
import { chainAdapters, contracts } from 'chainsig.js';

const NEAR_NETWORK_ID = 'testnet';
const VAULT_ACCOUNT_ID = process.env.NEAR_VAULT_ACCOUNT_ID || 'vault-heirvault01.testnet';
const VAULT_PRIVATE_KEY = process.env.NEAR_VAULT_PRIVATE_KEY;

if (!VAULT_PRIVATE_KEY) {
  console.error('Error: NEAR_VAULT_PRIVATE_KEY environment variable is required.');
  process.exit(1);
}

const BITCOIN_NETWORK = 'testnet';
const BLOCKSTREAM_API_URL = 'https://blockstream.info/testnet/api';

async function main() {
  console.log('=== FUNDING BITCOIN-1 FROM BITCOIN-2 LEFTOVER ===\n');

  const provider = new JsonRpcProvider({ url: 'https://rpc.testnet.near.org' });
  const keyPair = KeyPair.fromString(VAULT_PRIVATE_KEY);
  const signer = new KeyPairSigner(keyPair);
  const vaultAccount = new Account(VAULT_ACCOUNT_ID, provider, signer);

  const csContract = new contracts.ChainSignatureContract({
    networkId: NEAR_NETWORK_ID,
    contractId: 'v1.signer-prod.testnet'
  });

  const btcRpc = new chainAdapters.btc.Mempool(BLOCKSTREAM_API_URL);
  const btc = new chainAdapters.btc.Bitcoin({
    network: BITCOIN_NETWORK,
    contract: csContract,
    btcRpcAdapter: btcRpc
  });

  // Derive addresses
  const btc1 = await btc.deriveAddressAndPublicKey(VAULT_ACCOUNT_ID, 'bitcoin-1');
  const btc2 = await btc.deriveAddressAndPublicKey(VAULT_ACCOUNT_ID, 'bitcoin-2');

  console.log(`From (bitcoin-2): ${btc2.address}`);
  console.log(`To   (bitcoin-1): ${btc1.address}`);

  // Fetch UTXO from bitcoin-2
  const utxoRes = await fetch(`${BLOCKSTREAM_API_URL}/address/${btc2.address}/utxo`);
  const utxos = await utxoRes.json();
  if (!utxos || utxos.length === 0) {
    throw new Error(`No UTXO found at ${btc2.address}`);
  }
  const utxo = utxos[0];
  console.log(`Found UTXO on bitcoin-2: ${utxo.txid}:${utxo.vout} (${utxo.value} sats)`);

  const fee = 300;
  const sendAmount = utxo.value - fee;
  console.log(`Fee: ${fee} sats, Send Amount: ${sendAmount} sats`);

  const txRequest = {
    from: btc2.address,
    to: btc1.address,
    publicKey: btc2.publicKey,
    inputs: [{ txid: utxo.txid, vout: utxo.vout, value: utxo.value }],
    outputs: [{ address: btc1.address, value: sendAmount }]
  };

  console.log('\nPreparing transaction for signing with path bitcoin-2...');
  const { transaction, hashesToSign } = await btc.prepareTransactionForSigning(txRequest);

  console.log('Requesting MPC signature from v1.signer-prod.testnet...');
  const rsvSignatures = await csContract.sign({
    payloads: hashesToSign,
    path: 'bitcoin-2',
    keyType: 'Ecdsa',
    signerAccount: vaultAccount
  });
  console.log('Received MPC signature!');

  const signedTxHex = btc.finalizeTransactionSigning({ transaction, rsvSignatures });
  console.log('\nBroadcasting transaction to Bitcoin testnet...');

  // Broadcast to Blockstream
  const bcRes = await fetch(`${BLOCKSTREAM_API_URL}/tx`, {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: signedTxHex
  });

  if (!bcRes.ok) {
    const errText = await bcRes.text();
    throw new Error(`Broadcast failed (${bcRes.status}): ${errText}`);
  }

  const txid = await bcRes.text();
  console.log(`\n🎉 TRANSFER BROADCAST SUCCESSFUL!`);
  console.log(`- Txid: ${txid}`);
  console.log(`- Funded ${btc1.address} with ${sendAmount} sats (UTXO ${txid}:0)`);
}

main().catch(err => {
  console.error('\n❌ Funding failed:', err);
  process.exit(1);
});
