import { secp256k1 } from '@noble/curves/secp256k1';
import { bytesToNumberBE } from '@noble/curves/utils';
import { ripemd160 } from '@noble/hashes/legacy';
import { sha3_256 } from '@noble/hashes/sha3';
import { sha256 } from '@noble/hashes/sha2';
import { baseDecode } from 'near-api-js';
import { bech32 } from 'bech32';
import { getAddress, keccak256 } from 'viem';

const DERIVATION_DOMAIN = 'near-mpc-recovery v0.1.0 epsilon derivation:';

/** Derive the secp256k1 child key used by NEAR Chain Signatures. */
export function deriveChildPublicKey(rootPublicKey, predecessor, path) {
  const [scheme, encodedKey] = String(rootPublicKey).split(':', 2);
  if (scheme !== 'secp256k1' || !encodedKey || !predecessor || !path) {
    throw new Error('Invalid Chain Signatures public key or derivation input.');
  }

  const rootCoordinates = baseDecode(encodedKey);
  if (rootCoordinates.length !== 64) {
    throw new Error('The MPC root public key must contain 64 secp256k1 coordinate bytes.');
  }
  const rootBytes = new Uint8Array(65);
  rootBytes[0] = 4;
  rootBytes.set(rootCoordinates, 1);

  const derivationInput = new TextEncoder().encode(`${DERIVATION_DOMAIN}${predecessor},${path}`);
  const epsilon = bytesToNumberBE(sha3_256(derivationInput)) % secp256k1.Point.Fn.ORDER;
  if (epsilon === 0n) throw new Error('The derived Chain Signatures offset is invalid.');

  return secp256k1.Point.fromBytes(rootBytes)
    .add(secp256k1.Point.BASE.multiply(epsilon))
    .toBytes(false);
}

export function deriveEvmAddress(uncompressedPublicKey) {
  if (uncompressedPublicKey.length !== 65 || uncompressedPublicKey[0] !== 4) {
    throw new Error('Expected an uncompressed secp256k1 public key.');
  }
  const digest = keccak256(uncompressedPublicKey.subarray(1));
  return getAddress(`0x${digest.slice(-40)}`);
}

export function deriveBitcoinTestnetAddress(uncompressedPublicKey) {
  const point = secp256k1.Point.fromBytes(uncompressedPublicKey);
  const witnessProgram = ripemd160(sha256(point.toBytes(true)));
  return bech32.encode('tb', [0, ...bech32.toWords(witnessProgram)]);
}

/** Rebuild a native Bitcoin Testnet P2WPKH address from the 20-byte value stored by the contract. */
export function bitcoinTestnetWitnessProgramToAddress(value) {
  const hex = String(value || '').trim().replace(/^0x/i, '');
  if (!/^[0-9a-fA-F]{40}$/.test(hex)) {
    throw new Error('The saved Bitcoin destination is not a valid 20-byte witness program.');
  }
  const program = Uint8Array.from(hex.match(/.{2}/g), (byte) => Number.parseInt(byte, 16));
  return bech32.encode('tb', [0, ...bech32.toWords(program)]);
}

/** Convert a native Bitcoin Testnet P2WPKH address into the 20-byte program stored by the contract. */
export function bitcoinTestnetAddressToWitnessProgram(address) {
  let decoded;
  try {
    decoded = bech32.decode(String(address || '').trim());
  } catch {
    throw new Error('Invalid Bitcoin Testnet address. Use a native SegWit address beginning with tb1q.');
  }
  if (decoded.prefix !== 'tb' || decoded.words[0] !== 0) {
    throw new Error('Use a native SegWit Bitcoin Testnet address beginning with tb1q.');
  }
  const program = Uint8Array.from(bech32.fromWords(decoded.words.slice(1)));
  if (program.length !== 20) throw new Error('Bitcoin destination must be a P2WPKH address with a 20-byte witness program.');
  return Array.from(program, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
