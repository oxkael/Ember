import crypto from 'crypto';

function sha256(data) {
  return crypto.createHash('sha256').update(data).digest();
}
function doubleSha256(data) {
  return sha256(sha256(data));
}

export function computeBip143Sighash(txidHex, vout, senderPkhHex, valueSats, sequence, outputs, locktime = 0) {
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

// Test Vector 2:
const vec2Hash = computeBip143Sighash(
  '781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72',
  1,
  '77ed9969481823854b8c850b4596a16030bb3480',
  122462,
  0xfffffffd,
  [
    { valueSats: 61090, witnessProgramHex: '8914c1af8c64acb5e0080fd4de181d30e4456ad4' },
    { valueSats: 61090, witnessProgramHex: '77ed9969481823854b8c850b4596a16030bb3480' }
  ]
);

console.log('Computed Sighash Vector 2:', vec2Hash.toString('hex'));
console.log('Expected:                ', '111d2118913b13da5df4cacbd0df9628ae80f72a08fd823f027e53cbb224ba3a');
console.log('Match:                   ', vec2Hash.toString('hex') === '111d2118913b13da5df4cacbd0df9628ae80f72a08fd823f027e53cbb224ba3a');
