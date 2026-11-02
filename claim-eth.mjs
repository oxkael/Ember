import { ethers } from "ethers";
import { execSync } from "child_process";
import fs from "fs";

const SEPOLIA_RPC = "https://ethereum-sepolia-rpc.publicnode.com";
const provider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

const VAULT_CONTRACT = "vault-heirvault01.testnet";
const VAULT_ADDRESS = "0xDAe49575b025e1f21b3238605fE1887c97da210a";
const CHAIN_ID = 11155111;

const HEIRS = [
  {
    index: 0,
    accountId: "heir1-heirvault01.testnet",
    address: "0x5E7e7e39252a159e64a92161f99290E2bFDc42ee",
    shareBps: 5000,
  },
  {
    index: 1,
    accountId: "heir2-heirvault01.testnet",
    address: "0x8AFd2Ad161ff90424eD62A43f6dF6905d96a423d",
    shareBps: 5000,
  },
];

function getVaultStatus() {
  const statusCmd = `near contract call-function as-read-only ${VAULT_CONTRACT} status file-args "C:\\Users\\user\\Desktop\\Heir\\empty.json" network-config testnet now`;
  const output = execSync(statusCmd, { encoding: "utf8" });
  const startIdx = output.indexOf("{\n  \"btc_sender_pubkey_hash\"");
  const fallbackIdx = output.indexOf("{\n");
  const actualStart = startIdx !== -1 ? startIdx : fallbackIdx;
  const endIdx = output.lastIndexOf("}") + 1;
  return JSON.parse(output.substring(actualStart, endIdx));
}

async function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function processHeir(heir, initialPoolBalanceWei) {
  console.log(`\n======================================================`);
  console.log(`>>> PROCESSING HEIR #${heir.index} (${heir.accountId})`);
  console.log(`======================================================`);

  // 1. Fetch current on-chain nonce and balance
  const currentNonce = await provider.getTransactionCount(VAULT_ADDRESS, "pending");
  const currentVaultBal = await provider.getBalance(VAULT_ADDRESS);
  const heirBalBefore = await provider.getBalance(heir.address);

  console.log(`- Current Vault On-chain Nonce: ${currentNonce}`);
  console.log(`- Current Vault Balance: ${currentVaultBal.toString()} wei (${ethers.formatEther(currentVaultBal)} ETH)`);
  console.log(`- Heir Initial Balance: ${heirBalBefore.toString()} wei (${ethers.formatEther(heirBalBefore)} ETH)`);

  const gasLimit = 250000n;
  const gasPrice = 200000000n; // 0.2 gwei
  const feeWei = gasLimit * gasPrice; // 50,000,000,000,000 wei

  const heirGrossShare = (BigInt(initialPoolBalanceWei) * BigInt(heir.shareBps)) / 10000n;
  const transferValue = heirGrossShare - feeWei;

  console.log(`- Gross Share: ${heirGrossShare.toString()} wei (${ethers.formatEther(heirGrossShare)} ETH)`);
  console.log(`- Fee (gasLimit * gasPrice): ${feeWei.toString()} wei (${ethers.formatEther(feeWei)} ETH)`);
  console.log(`- Net Transfer Value: ${transferValue.toString()} wei (${ethers.formatEther(transferValue)} ETH)`);

  // 2. Build local unsigned transaction
  const txData = {
    type: 1, // EIP-2930
    to: heir.address,
    value: transferValue,
    nonce: currentNonce,
    gasLimit: gasLimit,
    gasPrice: gasPrice,
    chainId: CHAIN_ID,
    data: "0x",
  };

  const unsignedSerialized = ethers.Transaction.from(txData).unsignedSerialized;
  const localHashWith0x = ethers.keccak256(unsignedSerialized);
  const localHash = localHashWith0x.startsWith("0x") ? localHashWith0x.slice(2).toLowerCase() : localHashWith0x.toLowerCase();
  console.log(`- Locally built unsigned tx hash: ${localHash}`);

  // 3. Prepare claim arguments file
  const claimArgs = {
    heir_index: heir.index,
    balance_wei: initialPoolBalanceWei.toString(),
    nonce: currentNonce.toString(),
    gas_price_wei: gasPrice.toString(),
  };

  const argsFile = `C:\\Users\\user\\Desktop\\Heir\\claim-args-slot-${heir.index}.json`;
  fs.writeFileSync(argsFile, JSON.stringify(claimArgs, null, 2));

  // 4. Call `claim` on NEAR contract as the heir
  console.log(`\n- Invoking claim() on NEAR contract as ${heir.accountId}...`);
  const claimCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} claim file-args "${argsFile}" prepaid-gas "300.0 Tgas" attached-deposit "0.05 NEAR" sign-as ${heir.accountId} network-config testnet sign-with-keychain send`;
  console.log(`Executing: ${claimCmd}`);
  const claimOut = execSync(claimCmd, { encoding: "utf8" });
  console.log("claim() call submitted successfully!");

  // 5. Poll vault status until slot is "Signed"
  console.log(`\n- Polling contract status for slot #${heir.index} to become "Signed"...`);
  let slotStatus;
  let contractStoredHash;
  let mpcSigRaw;

  for (let attempt = 1; attempt <= 30; attempt++) {
    await sleep(4000);
    const status = getVaultStatus();
    const slot = status.heirs[heir.index];
    console.log(`  [Attempt ${attempt}] Slot status: ${slot.status}`);

    if (slot.status === "Signed") {
      slotStatus = slot;
      contractStoredHash = slot.signed_hash ? slot.signed_hash.toLowerCase() : null;
      mpcSigRaw = slot.tx_hash;
      break;
    }
  }

  if (!slotStatus || slotStatus.status !== "Signed") {
    throw new Error(`Slot #${heir.index} did not reach "Signed" state!`);
  }

  console.log(`- Contract Stored Hash: ${contractStoredHash}`);
  console.log(`- Locally Built Hash:   ${localHash}`);

  // 6. Confirm contract's stored hash equals the locally built one
  if (contractStoredHash !== localHash) {
    throw new Error(`CRITICAL MISMATCH: Contract stored hash (${contractStoredHash}) !== Local hash (${localHash})`);
  }
  console.log(`✅ HASH MATCH CONFIRMED: Contract hash matches local BIP/EIP-2930 hash exactly!`);

  // 7. Assemble signed transaction
  const mpcSig = JSON.parse(mpcSigRaw);
  const r = "0x" + mpcSig.big_r.affine_point.substring(2);
  const s = "0x" + mpcSig.s.scalar;
  const yParity = mpcSig.recovery_id;

  const signedTx = ethers.Transaction.from({
    ...txData,
    signature: { r, s, yParity },
  });

  console.log(`- Recovered Sender: ${signedTx.from}`);
  if (signedTx.from.toLowerCase() !== VAULT_ADDRESS.toLowerCase()) {
    throw new Error(`CRITICAL SENDER MISMATCH: Expected ${VAULT_ADDRESS}, got ${signedTx.from}`);
  }
  console.log(`✅ SENDER MATCH CONFIRMED: Signature recovers to vault address!`);

  // 8. Broadcast to Sepolia
  console.log(`\n- Broadcasting signed transaction to Sepolia...`);
  const broadcastRes = await provider.broadcastTransaction(signedTx.serialized);
  console.log(`🚀 Broadcasted! Tx Hash: ${broadcastRes.hash}`);
  const etherscanLink = `https://sepolia.etherscan.io/tx/${broadcastRes.hash}`;
  console.log(`Etherscan Link: ${etherscanLink}`);

  // 9. Wait for confirmation
  console.log(`- Waiting for block confirmation...`);
  const receipt = await broadcastRes.wait(1);
  console.log(`✅ Transaction mined in block #${receipt.blockNumber}! Gas Used: ${receipt.gasUsed.toString()}`);

  // 10. Call mark_done on NEAR contract
  console.log(`\n- Calling mark_done(${heir.index}) as ${heir.accountId}...`);
  const markArgsFile = `C:\\Users\\user\\Desktop\\Heir\\mark-done-${heir.index}.json`;
  fs.writeFileSync(markArgsFile, JSON.stringify({ heir_index: heir.index }, null, 2));

  const markCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} mark_done file-args "${markArgsFile}" prepaid-gas "100.0 Tgas" attached-deposit "0 NEAR" sign-as ${heir.accountId} network-config testnet sign-with-keychain send`;
  console.log(`Executing: ${markCmd}`);
  execSync(markCmd, { encoding: "utf8" });
  console.log(`✅ mark_done call succeeded!`);

  const heirBalAfter = await provider.getBalance(heir.address);
  console.log(`- Heir Final Balance: ${heirBalAfter.toString()} wei (${ethers.formatEther(heirBalAfter)} ETH)`);

  return {
    heirIndex: heir.index,
    heirAccount: heir.accountId,
    heirAddress: heir.address,
    txHash: broadcastRes.hash,
    etherscanLink,
    blockNumber: receipt.blockNumber,
    gasUsed: receipt.gasUsed.toString(),
    heirFinalBalance: heirBalAfter.toString(),
    heirFinalBalanceEth: ethers.formatEther(heirBalAfter),
  };
}

async function main() {
  console.log("=== ETHEREUM INHERITANCE CLAIM PIPELINE ===\n");

  // 1. Vault initial balance
  const initialVaultBalance = await provider.getBalance(VAULT_ADDRESS);
  console.log(`Vault Address: ${VAULT_ADDRESS}`);
  console.log(`Vault Initial Sepolia Balance: ${initialVaultBalance.toString()} wei (${ethers.formatEther(initialVaultBalance)} ETH)`);

  // 2. Check vault state
  let status = getVaultStatus();
  console.log(`Vault Contract State: ${status.state}`);
  if (status.state !== "Claimable") {
    throw new Error(`Vault is not Claimable yet! Current state: ${status.state}`);
  }

  // 3. Process Heir 1
  const res1 = await processHeir(HEIRS[0], initialVaultBalance);

  // 4. Process Heir 2
  const res2 = await processHeir(HEIRS[1], initialVaultBalance);

  // 5. Final leftover balance
  const leftoverBal = await provider.getBalance(VAULT_ADDRESS);
  console.log("\n========================================================");
  console.log("=== ALL CLAIMS COMPLETED SUCCESSFULLY ===");
  console.log("========================================================");
  console.log(`Vault Sepolia Balance Before: ${initialVaultBalance.toString()} wei (${ethers.formatEther(initialVaultBalance)} ETH)`);
  console.log(`Heir 1 (${res1.heirAddress}): Block #${res1.blockNumber}, Gas Used: ${res1.gasUsed}, Link: ${res1.etherscanLink}`);
  console.log(`Heir 1 Final Balance: ${res1.heirFinalBalance} wei (${res1.heirFinalBalanceEth} ETH)`);
  console.log(`Heir 2 (${res2.heirAddress}): Block #${res2.blockNumber}, Gas Used: ${res2.gasUsed}, Link: ${res2.etherscanLink}`);
  console.log(`Heir 2 Final Balance: ${res2.heirFinalBalance} wei (${res2.heirFinalBalanceEth} ETH)`);
  console.log(`Vault Leftover Balance: ${leftoverBal.toString()} wei (${ethers.formatEther(leftoverBal)} ETH)`);
}

main().catch(err => {
  console.error("FATAL ERROR in pipeline:", err);
  process.exit(1);
});
