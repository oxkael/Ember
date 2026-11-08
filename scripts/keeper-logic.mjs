export function enumName(value) {
  return typeof value === "string" ? value : Object.keys(value ?? {})[0] ?? "Unknown";
}

export function hasUnfinishedClaimWork(status) {
  const ethUnfinished = Array.isArray(status?.heirs) && status.heirs.some((slot) => enumName(slot.status) !== "Done");
  const btcUnfinished = enumName(status?.btc_slot?.status) !== "Done";
  return ethUnfinished || btcUnfinished;
}

export function hasUnfinishedEthWork(status) {
  return Array.isArray(status?.heirs) && status.heirs.some((slot) => enumName(slot.status) !== "Done");
}

export function selectOwnerIds(owners, requestedOwnerId = "") {
  const ownerId = requestedOwnerId.trim();
  if (!ownerId) return owners;
  if (!owners.includes(ownerId)) {
    throw new Error(`Requested owner ${ownerId} is not present in the contract owner registry.`);
  }
  return [ownerId];
}

export function parseMpcSignature(raw) {
  if (typeof raw !== "string") throw new Error("Contract did not return a serialized MPC signature.");
  let sig;
  try { sig = JSON.parse(raw); } catch { throw new Error("Stored MPC signature is not valid JSON."); }
  const point = sig?.big_r?.affine_point;
  const scalar = sig?.s?.scalar;
  const recovery = Number(sig?.recovery_id);
  if (typeof point !== "string" || !/^(02|03)[0-9a-f]{64}$/i.test(point) ||
      typeof scalar !== "string" || !/^[0-9a-f]{64}$/i.test(scalar) || ![0, 1].includes(recovery)) {
    throw new Error("Stored MPC signature has an unexpected shape.");
  }
  // NEAR returns big_r as a compressed point; r is its x coordinate (the 32 bytes after 02/03).
  return { r: `0x${point.slice(2)}`, s: `0x${scalar}`, yParity: recovery };
}

export function calculateBtcOutputs(inputSats, feeRate, heirs) {
  const input = BigInt(inputSats);
  const rate = BigInt(feeRate);
  if (!heirs.length) throw new Error("Cannot build Bitcoin outputs without recipients.");
  const vbytes = 11n + 68n + 31n * BigInt(heirs.length);
  const fee = vbytes * rate;
  if (rate <= 0n || rate > 200n || input <= fee || fee * 100n > input * 5n) throw new Error("BTC UTXO/fee does not satisfy the contract limits.");
  const pool = input - fee;
  let allocated = 0n;
  return heirs.map((slot, index) => {
    const program = slot.heir.bitcoin_witness_program;
    if (!program || !/^[0-9a-f]{40}$/i.test(program)) throw new Error("A recipient is missing a valid Bitcoin witness program.");
    const value = index === heirs.length - 1 ? pool - allocated : pool * BigInt(slot.heir.share_basis_points) / 10000n;
    if (value < 294n) throw new Error("BTC output falls below the contract dust limit.");
    allocated += value;
    return { value, program };
  });
}
