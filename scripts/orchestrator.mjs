import { ethers } from "ethers";
import { execSync } from "child_process";
import fs from "fs";

const SEPOLIA_RPC = "https://ethereum-sepolia-rpc.publicnode.com";
const provider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

const VAULT_CONTRACT = "vault-heirvault01.testnet";
const VAULT_ADDRESS = "0xDAe49575b025e1f21b3238605fE1887c97da210a";
const CHAIN_ID = 11155111;

function getVaultStatus() {
  const statusCmd = `near contract call-function as-read-only ${VAULT_CONTRACT} status file-args "C:\\Users\\user\\Desktop\\Heir\\empty.json" network-config testnet now`;
  const output = execSync(statusCmd, { encoding: "utf8" });
  const startIdx = output.indexOf("{\n  \"current_timestamp_ns\"");
  const endIdx = output.lastIndexOf("}") + 1;
  return JSON.parse(output.substring(startIdx, endIdx));
}

export async function processSlot(slotIndex) {
  console.log(`\n============================================================`);
  console.log(`PREPARING CLAIM FOR HEIR SLOT #${slotIndex}`);
  console.log(`============================================================`);

  const status = getVaultStatus();
  console.log(`Vault State: ${status.state}`);
  if (status.state !== "Claimable") {
    throw new Error(`Vault is not Claimable! Current state: ${status.state}`);
  }

  const slot = status.heirs[slotIndex];
  const heirNearAccount = slot.heir.near_account_id;
  const heirEthAddress = slot.heir.ethereum_address;
  const gasLimit = BigInt(status.gas_limit);

  console.log(`- Heir NEAR Signer: ${heirNearAccount}`);
  console.log(`- Heir Target Eth:  ${heirEthAddress}`);
  console.log(`- Configured Gas Limit: ${gasLimit.toString()}`);

  const balance = await provider.getBalance(VAULT_ADDRESS);
  const nonce = await provider.getTransactionCount(VAULT_ADDRESS, "pending");
  const feeData = await provider.getFeeData();
  const gasPrice = feeData.gasPrice || 100000000n;

  console.log(`\nVault Sepolia Balance: ${ethers.formatEther(balance)} ETH (${balance.toString()} wei)`);
  console.log(`Current Nonce: ${nonce}`);
  console.log(`Gas Price: ${gasPrice.toString()} wei (${ethers.formatUnits(gasPrice, "gwei")} gwei)`);

  // Use snapshot balance if already taken, otherwise current balance
  const snapshotBalance = status.snapshot_balance_wei ? BigInt(status.snapshot_balance_wei) : balance;
  console.log(`Snapshot Balance Used: ${ethers.formatEther(snapshotBalance)} ETH (${snapshotBalance.toString()} wei)`);

  const feeWei = gasLimit * gasPrice;
  const heirGrossShare = (snapshotBalance * BigInt(slot.heir.share_basis_points)) / 10000n;
  const transferValue = heirGrossShare - feeWei;

  console.log(`Fee Wei: ${feeWei.toString()} (${ethers.formatEther(feeWei)} ETH)`);
  console.log(`Heir Gross Share: ${heirGrossShare.toString()} (${ethers.formatEther(heirGrossShare)} ETH)`);
  console.log(`Transfer Value: ${transferValue.toString()} (${ethers.formatEther(transferValue)} ETH)`);

  // Build expected local EIP-2930 unsigned transaction
  const txData = {
    type: 1,
    chainId: CHAIN_ID,
    nonce: nonce,
    gasPrice: gasPrice,
    gasLimit: gasLimit,
    to: heirEthAddress,
    value: transferValue,
    data: "0x",
    accessList: []
  };

  const unsignedSerialized = ethers.Transaction.from(txData).unsignedSerialized;
  const localUnsignedHash = ethers.keccak256(unsignedSerialized);
  console.log(`\nLocal Unsigned Serialized: ${unsignedSerialized}`);
  console.log(`Local Unsigned Tx Hash:   ${localUnsignedHash}`);

  // Prepare claim arguments (no tx_payload)
  const claimArgs = {
    heir_index: slotIndex,
    balance_wei: balance.toString(),
    nonce: nonce.toString(),
    gas_price_wei: gasPrice.toString()
  };

  const claimArgsPath = `C:\\Users\\user\\Desktop\\Heir\\claim-slot-${slotIndex}.json`;
  fs.writeFileSync(claimArgsPath, JSON.stringify(claimArgs, null, 2));

  console.log(`\nCalling claim() signed by HEIR (${heirNearAccount})...`);
  const claimCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} claim file-args "${claimArgsPath}" prepaid-gas "300.0 Tgas" attached-deposit "1 yoctoNEAR" sign-as ${heirNearAccount} network-config testnet sign-with-keychain send`;
  const claimOutput = execSync(claimCmd, { encoding: "utf8" });
  console.log(`Claim call completed!`);
  console.log(claimOutput);

  // Read updated status
  const updatedStatus = getVaultStatus();
  const updatedSlot = updatedStatus.heirs[slotIndex];
  console.log(`\nUpdated Slot Status: ${updatedSlot.status}`);
  console.log(`Contract Stored Signed Hash: 0x${updatedSlot.signed_hash}`);
  console.log(`Local Unsigned Tx Hash:      ${localUnsignedHash}`);

  // Enforce match
  const hashesMatch = (`0x${updatedSlot.signed_hash}`.toLowerCase() === localUnsignedHash.toLowerCase());
  console.log(`Hashes match?: ${hashesMatch ? "YES ✅ MATCH CONFIRMED" : "NO ❌ MISMATCH"}`);
  if (!hashesMatch) {
    throw new Error(`Hash mismatch! Contract signed 0x${updatedSlot.signed_hash} but local tx is ${localUnsignedHash}`);
  }

  // Parse signature
  const mpcSig = JSON.parse(updatedSlot.tx_hash);
  const r = "0x" + mpcSig.big_r.affine_point.substring(2);
  const s = "0x" + mpcSig.s.scalar;
  const yParity = mpcSig.recovery_id;

  const signedTx = ethers.Transaction.from({
    ...txData,
    signature: { r, s, yParity }
  });

  console.log(`\nRecovered Signer / From: ${signedTx.from}`);
  console.log(`Matches Vault?: ${signedTx.from.toLowerCase() === VAULT_ADDRESS.toLowerCase() ? "YES ✅" : "NO ❌"}`);

  return { signedTx, slotIndex, heirNearAccount };
}

export async function broadcastAndMarkDone({ signedTx, slotIndex, heirNearAccount }) {
  console.log(`\nBroadcasting Slot #${slotIndex} transaction to Sepolia testnet...`);
  const broadcastRes = await provider.broadcastTransaction(signedTx.serialized);
  console.log(`🚀 Broadcasted Tx Hash: ${broadcastRes.hash}`);
  console.log(`Sepolia Etherscan: https://sepolia.etherscan.io/tx/${broadcastRes.hash}`);

  console.log(`Waiting for confirmation...`);
  const receipt = await broadcastRes.wait(1);
  console.log(`✅ Confirmed in block #${receipt.blockNumber}! Status: ${receipt.status}`);

  console.log(`\nCalling mark_done() signed by HEIR (${heirNearAccount})...`);
  const markDoneArgsPath = `C:\\Users\\user\\Desktop\\Heir\\mark-done-${slotIndex}.json`;
  fs.writeFileSync(markDoneArgsPath, JSON.stringify({ heir_index: slotIndex }));
  const markCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} mark_done file-args "${markDoneArgsPath}" prepaid-gas "100.0 Tgas" attached-deposit "0 NEAR" sign-as ${heirNearAccount} network-config testnet sign-with-keychain send`;
  const markOutput = execSync(markCmd, { encoding: "utf8" });
  console.log(`mark_done completed!`);
  console.log(markOutput);

  const finalStatus = getVaultStatus();
  console.log(`Final Slot #${slotIndex} Status: ${finalStatus.heirs[slotIndex].status}`);
}
