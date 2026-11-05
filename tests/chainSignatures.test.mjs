import assert from 'node:assert/strict';
import test from 'node:test';
import {
  bitcoinTestnetAddressToWitnessProgram,
  deriveBitcoinTestnetAddress,
  deriveChildPublicKey,
  deriveEvmAddress,
} from '../frontend/src/chainSignatures.js';

const TESTNET_MPC_ROOT = 'secp256k1:4NfTiv3UsGahebgTaHyD9vF8KYKMBnfd6kh94mK6xv8fGBiJB8TBtFMP5WWXz6B89Ac1fbpzPwAvoyQebemHFwx3';
const CONTRACT_ID = 'ember.testnet';
const OWNER_ID = 'alice.testnet';

test('matches the NEAR Chain Signatures EVM address derivation vector', () => {
  const path = `ember/v1/${OWNER_ID}/ethereum`;
  const publicKey = deriveChildPublicKey(TESTNET_MPC_ROOT, CONTRACT_ID, path);
  assert.equal(deriveEvmAddress(publicKey), '0xe991248fBe5728a9b70BC4c98bd5875F5432821a');
});

test('matches the NEAR Chain Signatures Bitcoin testnet address derivation vector', () => {
  const path = `ember/v1/${OWNER_ID}/bitcoin`;
  const publicKey = deriveChildPublicKey(TESTNET_MPC_ROOT, CONTRACT_ID, path);
  assert.equal(deriveBitcoinTestnetAddress(publicKey), 'tb1qek40evjfl59cfxzvpn2ps43ftxdg5trzvw5sth');
  assert.equal(bitcoinTestnetAddressToWitnessProgram('tb1qek40evjfl59cfxzvpn2ps43ftxdg5trzvw5sth'), 'cdaafcb249fd0b84984c0cd4185629599a8a2c62');
});

test('rejects Bitcoin addresses outside native SegWit Testnet P2WPKH', () => {
  assert.throws(() => bitcoinTestnetAddressToWitnessProgram('bc1qek40evjfl59cfxzvpn2ps43ftxdg5trzvw5sth'), /Bitcoin Testnet/);
  assert.throws(() => bitcoinTestnetAddressToWitnessProgram('not-an-address'), /Invalid Bitcoin Testnet address/);
});
