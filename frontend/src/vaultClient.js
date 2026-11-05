import { JsonRpcProvider } from 'near-api-js';
import { MPC_CONTRACT_ID, VAULT_ACCOUNT_ID } from './config.js';
import { bitcoinTestnetAddressToWitnessProgram, deriveBitcoinTestnetAddress, deriveChildPublicKey, deriveEvmAddress } from './chainSignatures.js';

// Keep the checkpoint contract address and read interface in this adapter.
const provider = new JsonRpcProvider({ url: 'https://rpc.testnet.near.org' });

function validateAccountId(value, label) {
  const accountId = String(value || '').trim();
  if (!/^[a-z0-9._-]+$/.test(accountId) || accountId.length < 2 || accountId.length > 64) {
    throw new Error(`Configure a valid NEAR testnet ${label} account ID.`);
  }
  return accountId;
}

export async function readVaultStatus(ownerId, contractId = VAULT_ACCOUNT_ID) {
  const value = String(contractId).trim();
  const validContractId = validateAccountId(value, 'Ember contract');
  const validOwnerId = validateAccountId(ownerId, 'connected wallet');
  return provider.callFunction({
    contractId: validContractId,
    method: 'status',
    args: { owner_id: validOwnerId },
  });
}

export async function deriveVaultDepositAddresses(ownerId, contractId = VAULT_ACCOUNT_ID) {
  const validContractId = validateAccountId(contractId, 'Ember contract');
  const validOwnerId = validateAccountId(ownerId, 'connected wallet');
  const ethereumPath = `ember/v1/${validOwnerId}/ethereum`;
  const bitcoinPath = `ember/v1/${validOwnerId}/bitcoin`;
  const mpcRootPublicKey = await provider.callFunction({
    contractId: MPC_CONTRACT_ID,
    method: 'public_key',
    args: {},
  });
  const ethereumKey = deriveChildPublicKey(mpcRootPublicKey, validContractId, ethereumPath);
  const bitcoinKey = deriveChildPublicKey(mpcRootPublicKey, validContractId, bitcoinPath);

  return {
    ethereum: { address: deriveEvmAddress(ethereumKey), path: ethereumPath },
    bitcoin: { address: deriveBitcoinTestnetAddress(bitcoinKey), path: bitcoinPath },
  };
}

export async function setupVault(wallet, ownerId, heirs, heartbeatIntervalSec, gracePeriodSec, keeperId, depositAddresses, contractId = VAULT_ACCOUNT_ID) {
  const validContractId = validateAccountId(contractId, 'Ember contract');
  const validOwnerId = validateAccountId(ownerId, 'connected wallet');
  const args = {
    heirs: heirs.map((recipient) => ({
      near_account_id: validateAccountId(recipient.nearAccountId, 'destination NEAR account'),
      ethereum_address: String(recipient.ethereumAddress || '').trim(),
      bitcoin_witness_program: bitcoinTestnetAddressToWitnessProgram(recipient.bitcoinAddress),
      share_basis_points: Math.round(Number(recipient.sharePercent) * 100),
    })),
    heartbeat_interval_sec: Math.round(Number(heartbeatIntervalSec)),
    grace_period_sec: Math.round(Number(gracePeriodSec)),
    gas_limit: null,
    btc_sender_pubkey_hash: bitcoinTestnetAddressToWitnessProgram(depositAddresses.bitcoin.address),
    keeper_id: keeperId ? validateAccountId(keeperId, 'keeper') : null,
  };
  const totalShares = args.heirs.reduce((sum, recipient) => sum + recipient.share_basis_points, 0);
  if (totalShares !== 10000) throw new Error('Destination shares must add up to 100%.');
  for (const recipient of args.heirs) {
    if (!/^0x[0-9a-fA-F]{40}$/.test(recipient.ethereum_address)) throw new Error('Enter a valid Ethereum Sepolia address for every destination.');
  }
  for (const seconds of [args.heartbeat_interval_sec, args.grace_period_sec]) {
    if (!Number.isSafeInteger(seconds) || seconds < 1 || seconds > 31_536_000) throw new Error('Check-in and grace periods must be between 1 second and 365 days.');
  }
  await signContractCall(wallet, validOwnerId, validContractId, 'setup', args, '300000000000000');
}

export async function pingVault(wallet, ownerId, contractId = VAULT_ACCOUNT_ID) {
  await signContractCall(wallet, ownerId, validateAccountId(contractId, 'Ember contract'), 'ping', {}, '100000000000000');
}

async function signContractCall(wallet, ownerId, contractId, methodName, args, gas) {
  if (!wallet?.signAndSendTransaction) throw new Error('The connected wallet cannot submit NEAR transactions.');
  const outcome = await wallet.signAndSendTransaction({
    network: 'testnet',
    signerId: ownerId,
    receiverId: contractId,
    actions: [{
      type: 'FunctionCall',
      params: { methodName, args, gas, deposit: '0' },
    }],
  });
  if (outcome?.status?.Failure || outcome?.status === 'Failure') {
    throw new Error(`NEAR transaction failed: ${JSON.stringify(outcome.status.Failure || outcome.status)}`);
  }
  return outcome;
}
