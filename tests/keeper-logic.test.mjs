import test from "node:test";
import assert from "node:assert/strict";
import * as bitcoin from "bitcoinjs-lib";
import { ethers } from "ethers";
import { calculateBtcOutputs, enumName, parseMpcSignature } from "../scripts/keeper-logic.mjs";

const recipients = [
  { heir: { bitcoin_witness_program: "11".repeat(20), share_basis_points: 7000 } },
  { heir: { bitcoin_witness_program: "22".repeat(20), share_basis_points: 3000 } },
];

test("BTC output amounts match contract fee, shares, and remainder allocation", () => {
  const outputs = calculateBtcOutputs(100_000n, 2n, recipients);
  assert.deepEqual(outputs, [
    { value: 69_802n, program: "11".repeat(20) },
    { value: 29_916n, program: "22".repeat(20) },
  ]);
});

test("BTC output calculation rejects an over-cap fee and missing recipient program", () => {
  assert.throws(() => calculateBtcOutputs(10_000n, 200n, recipients), /contract limits/);
  assert.throws(() => calculateBtcOutputs(100_000n, 2n, [{ heir: { share_basis_points: 10_000 } }]), /missing a valid Bitcoin witness program/);
});

test("MPC signature parser extracts r from compressed big_r and rejects malformed data", () => {
  const x = "ab".repeat(32);
  assert.deepEqual(parseMpcSignature(JSON.stringify({
    big_r: { affine_point: `02${x}` }, s: { scalar: "cd".repeat(32) }, recovery_id: 1,
  })), { r: `0x${x}`, s: `0x${"cd".repeat(32)}`, yParity: 1 });
  assert.throws(() => parseMpcSignature("not-json"), /not valid JSON/);
  assert.throws(() => parseMpcSignature(JSON.stringify({ big_r: { affine_point: "04" }, s: { scalar: "00" }, recovery_id: 2 })), /unexpected shape/);
});

test("enumName handles NEAR JSON enum serialization", () => {
  assert.equal(enumName("Signed"), "Signed");
  assert.equal(enumName({ Bitcoin: {} }), "Bitcoin");
  assert.equal(enumName(null), "Unknown");
});

test("Bitcoin transaction builder reproduces the contract BIP-143 sighash vector", () => {
  const tx = new bitcoin.Transaction();
  tx.version = 2;
  tx.addInput(Buffer.from("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72", "hex").reverse(), 1, 0xfffffffd);
  const destination = bitcoin.payments.p2wpkh({
    hash: Buffer.from("8914c1af8c64acb5e0080fd4de181d30e4456ad4", "hex"),
    network: bitcoin.networks.testnet,
  });
  tx.addOutput(destination.output, 61090);
  tx.addOutput(bitcoin.payments.p2wpkh({
    hash: Buffer.from("77ed9969481823854b8c850b4596a16030bb3480", "hex"),
    network: bitcoin.networks.testnet,
  }).output, 61090);
  const sighash = tx.hashForWitnessV0(
    0,
    Buffer.from("76a91477ed9969481823854b8c850b4596a16030bb348088ac", "hex"),
    122462,
    bitcoin.Transaction.SIGHASH_ALL,
  );
  assert.equal(Buffer.from(sighash).toString("hex"), "111d2118913b13da5df4cacbd0df9628ae80f72a08fd823f027e53cbb224ba3a");
});

test("Ethereum unsigned transaction matches the contract EIP-2930 vector", () => {
  const tx = ethers.Transaction.from({
    type: 1,
    chainId: 11155111,
    nonce: 2,
    gasPrice: 200_000_000n,
    gasLimit: 250_000n,
    to: "0x3535353535353535353535353535353535353535",
    value: 20_000_000_000_000_000n,
    data: "0x",
    accessList: [],
  });
  assert.equal(tx.unsignedSerialized.slice(2), "01ed83aa36a702840bebc2008303d09094353535353535353535353535353535353535353587470de4df82000080c0");
  assert.equal(ethers.keccak256(tx.unsignedSerialized).slice(2), "e88cf8dc4168bc8347df111e1b49e7f37a9cc9fc7fa32c61d17ce497e3ae4941");
});
