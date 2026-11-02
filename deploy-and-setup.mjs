import { Account, JsonRpcProvider, KeyPairSigner, KeyPair } from 'near-api-js';
import fs from 'fs';
import path from 'path';

const RPC_URL = 'https://rpc.testnet.near.org';
const FAUCET_URL = 'https://helper.testnet.near.org/account';

const VAULT_ID = 'vault-heirvault01.testnet';
const OWNER_ID = 'owner-heirvault01.testnet';
const HEIR1_ID = 'heir1-heirvault01.testnet';
const HEIR2_ID = 'heir2-heirvault01.testnet';

// Active key pair for vault account
const VAULT_PRIVKEY = process.env.NEAR_VAULT_PRIVATE_KEY;
if (!VAULT_PRIVKEY) {
  console.error("Error: NEAR_VAULT_PRIVATE_KEY environment variable required");
  process.exit(1);
}
const keyPair = KeyPair.fromString(VAULT_PRIVKEY);
const vaultPubKey = keyPair.getPublicKey().toString();

const provider = new JsonRpcProvider({ url: RPC_URL });
const signer = new KeyPairSigner(keyPair);
const vaultAccount = new Account(VAULT_ID, provider, signer);

async function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}

async function main() {
  console.log('=== STEP 3: DEPLOY & SETUP ===\n');

  // 1. Delete vault account
  console.log(`[1/5] Deleting account ${VAULT_ID} with beneficiary ${OWNER_ID}...`);
  try {
    const delRes = await vaultAccount.deleteAccount(OWNER_ID);
    console.log('Delete tx hash:', delRes.transaction.hash);
  } catch (err) {
    console.log('Delete error (might not exist):', err.message);
  }

  await sleep(3000);

  // 2. Recreate vault account via faucet service
  console.log(`\n[2/5] Recreating account ${VAULT_ID} with public key ${vaultPubKey}...`);
  const createRes = await fetch(FAUCET_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      newAccountId: VAULT_ID,
      newAccountPublicKey: vaultPubKey
    })
  });
  const createData = await createRes.json();
  if (createData.transaction) {
    console.log('Recreation successful! Tx hash:', createData.transaction.hash);
  } else {
    console.log('Recreation response:', JSON.stringify(createData));
  }

  await sleep(4000);

  // 3. Deploy contract WASM
  const wasmPath = path.resolve('vault/target/near/vault.wasm');
  console.log(`\n[3/5] Deploying contract WASM from ${wasmPath}...`);
  const wasmBytes = fs.readFileSync(wasmPath);
  console.log(`WASM size: ${wasmBytes.length} bytes`);

  const deployRes = await vaultAccount.deployContract(wasmBytes);
  console.log('Deploy tx hash:', deployRes.transaction.hash);

  await sleep(2000);

  // 4. Call `new` initialization
  console.log('\n[4/5] Calling `new` initialization...');
  const initArgs = {
    owner_id: OWNER_ID,
    mpc_contract: 'v1.signer-prod.testnet',
    derivation_path: 'ethereum-1',
    gas_limit: '250000',
    btc_sender_pubkey_hash: '77ed9969481823854b8c850b4596a16030bb3480'
  };
  console.log('Init args:', JSON.stringify(initArgs, null, 2));

  const initRes = await vaultAccount.functionCall({
    contractId: VAULT_ID,
    methodName: 'new',
    args: initArgs,
    gas: '100000000000000', // 100 TGas
    attachedDeposit: '0'
  });
  console.log('`new` tx hash:', initRes.transaction.hash);

  console.log('\n=== Contract successfully deployed & initialized! ===');
}

main().catch(console.error);
