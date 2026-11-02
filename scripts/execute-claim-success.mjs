import { ethers } from "ethers";
import { execSync } from "child_process";
import fs from "fs";

const SEPOLIA_RPC = "https://ethereum-sepolia-rpc.publicnode.com";
const provider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

const VAULT_CONTRACT = "vault-heirvault01.testnet";
const OWNER_ACCOUNT = "owner-heirvault01.testnet";
const VAULT_ADDRESS = "0xDAe49575b025e1f21b3238605fE1887c97da210a";
const HEIR_ADDRESS = "0x849A9218b6daC5bad1b2411d1708b644f54ceA62";
const CHAIN_ID = 11155111;

function getVaultStatus() {
  const statusCmd = `near contract call-function as-read-only ${VAULT_CONTRACT} status file-args "C:\\Users\\user\\Desktop\\Heir\\empty.json" network-config testnet now`;
  const output = execSync(statusCmd, { encoding: "utf8" });
  const startIdx = output.indexOf("{\n  \"current_timestamp_ns\"");
  const endIdx = output.lastIndexOf("}") + 1;
  return JSON.parse(output.substring(startIdx, endIdx));
}

async function main() {
  console.log("=== EXECUTING CLEAN SEPOLIA INHERITANCE CLAIM ===");

  // 1. Check / wait for Claimable state
  let status = getVaultStatus();
  console.log(`Current Vault State: ${status.state}`);
  console.log(`Seconds until claimable: ${status.seconds_until_claimable}`);

  while (status.state !== "Claimable") {
    console.log(`Waiting for Claimable state... (currently: ${status.state}, remaining: ${status.seconds_until_claimable}s)`);
    await new Promise(r => setTimeout(r, 4000));
    status = getVaultStatus();
  }
  console.log(`Vault is ready and CLAIMABLE!`);

  // 2. Fetch Sepolia on-chain data
  const balance = await provider.getBalance(VAULT_ADDRESS);
  const nonce = await provider.getTransactionCount(VAULT_ADDRESS, "pending");
  console.log(`\nVault Sepolia Balance: ${ethers.formatEther(balance)} ETH (${balance.toString()} wei)`);
  console.log(`Vault Nonce: ${nonce}`);

  const heirInitialBal = await provider.getBalance(HEIR_ADDRESS);
  console.log(`Heir Initial Balance: ${ethers.formatEther(heirInitialBal)} ETH`);

  // 3. Build transaction with sufficient gas limit (250,000 gas)
  const gasLimit = 250000n;
  const gasPrice = 200000000n; // 0.2 gwei
  const sendValue = ethers.parseEther("0.02"); // 0.02 ETH native transfer

  console.log(`\nTransaction Parameters:`);
  console.log(`- Recipient (Heir): ${HEIR_ADDRESS}`);
  console.log(`- Amount: ${ethers.formatEther(sendValue)} ETH`);
  console.log(`- Gas Limit: ${gasLimit.toString()}`);
  console.log(`- Gas Price: ${gasPrice.toString()} wei (0.2 gwei)`);
  console.log(`- Nonce: ${nonce}`);

  const txData = {
    type: 1, // EIP-2930
    to: HEIR_ADDRESS,
    value: sendValue,
    nonce: nonce,
    gasLimit: gasLimit,
    gasPrice: gasPrice,
    chainId: CHAIN_ID,
    data: "0x",
  };

  const unsignedSerialized = ethers.Transaction.from(txData).unsignedSerialized;
  const txPayloadHash = ethers.keccak256(unsignedSerialized);
  console.log(`\nUnsigned Tx Payload Hash for MPC: ${txPayloadHash}`);

  const payloadBytes = Array.from(ethers.getBytes(txPayloadHash));

  // Write args file for NEAR CLI
  const claimArgs = {
    heir_index: 0,
    tx_payload: payloadBytes,
    balance_wei: balance.toString(),
    _nonce: nonce.toString(),
    gas_price_wei: gasPrice.toString(),
  };

  const argsPath = "C:\\Users\\user\\Desktop\\Heir\\claim-args-clean.json";
  fs.writeFileSync(argsPath, JSON.stringify(claimArgs, null, 2));

  // 4. Call NEAR claim()
  console.log(`\nInvoking claim() on NEAR contract ${VAULT_CONTRACT} with 300 Tgas...`);
  const claimCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} claim file-args "${argsPath}" prepaid-gas "300 Tgas" attached-deposit "1 yoctoNEAR" sign-as ${OWNER_ACCOUNT} network-config testnet sign-with-keychain send`;
  const claimOutput = execSync(claimCmd, { encoding: "utf8" });
  console.log("NEAR claim() function call succeeded!");

  // 5. Retrieve MPC signature from contract state
  console.log("\nReading MPC signature from contract slot #0...");
  status = getVaultStatus();
  const slot0 = status.heirs[0];
  console.log(`Slot #0 Status: ${slot0.status}`);
  console.log(`Slot #0 Signature Data: ${slot0.tx_hash}`);

  if (slot0.status !== "Done" || !slot0.tx_hash) {
    throw new Error("MPC signing did not complete successfully!");
  }

  const mpcSig = JSON.parse(slot0.tx_hash);
  const r = "0x" + mpcSig.big_r.affine_point.substring(2);
  const s = "0x" + mpcSig.s.scalar;
  const yParity = mpcSig.recovery_id;

  // 6. Assemble signed transaction
  const signedTx = ethers.Transaction.from({
    ...txData,
    signature: { r, s, yParity },
  });

  console.log("\nAssembled Signed Transaction:");
  console.log(`- From Address: ${signedTx.from}`);
  console.log(`- Matches Vault? ${signedTx.from.toLowerCase() === VAULT_ADDRESS.toLowerCase() ? "YES ✅" : "NO ❌"}`);
  console.log(`- Raw Serialized: ${signedTx.serialized}`);

  if (signedTx.from.toLowerCase() !== VAULT_ADDRESS.toLowerCase()) {
    throw new Error(`Sender mismatch! Expected ${VAULT_ADDRESS}, got ${signedTx.from}`);
  }

  // 7. Broadcast transaction to Sepolia
  console.log("\nBroadcasting signed transaction to Sepolia testnet...");
  const broadcastRes = await provider.broadcastTransaction(signedTx.serialized);
  console.log(`🚀 Transaction Broadcasted! Hash: ${broadcastRes.hash}`);
  console.log(`Sepolia Etherscan: https://sepolia.etherscan.io/tx/${broadcastRes.hash}`);

  // 8. Wait for block confirmation
  console.log("\nWaiting for Sepolia block confirmation...");
  const receipt = await broadcastRes.wait(1);
  console.log("\n========================================================");
  console.log(`🎉 TRANSACTION MINED IN BLOCK #${receipt.blockNumber}!`);
  console.log(`Receipt Status: ${receipt.status === 1 ? "1 (SUCCESS ✅)" : "0 (REVERTED ❌)"}`);
  console.log(`Gas Used: ${receipt.gasUsed.toString()} / ${gasLimit.toString()}`);
  console.log(`Effective Gas Price: ${ethers.formatUnits(receipt.effectiveGasPrice, "gwei")} gwei`);
  console.log("========================================================");

  // 9. Check final balances
  const vaultFinalBal = await provider.getBalance(VAULT_ADDRESS);
  const heirFinalBal = await provider.getBalance(HEIR_ADDRESS);
  console.log(`\nFinal Vault Balance: ${ethers.formatEther(vaultFinalBal)} ETH`);
  console.log(`Final Heir Balance:  ${ethers.formatEther(heirFinalBal)} ETH`);
  console.log(`Net Received by Heir: ${ethers.formatEther(heirFinalBal - heirInitialBal)} ETH`);
}

main().catch(console.error);
