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
  console.log("=== EXECUTING CLAIM FOR SLOT #1 ===");

  const balance = await provider.getBalance(VAULT_ADDRESS);
  const nonce = await provider.getTransactionCount(VAULT_ADDRESS, "pending");
  console.log(`Vault Balance: ${ethers.formatEther(balance)} ETH`);
  console.log(`Vault Nonce: ${nonce}`);

  const gasLimit = 250000n;
  const gasPrice = 200000000n; // 0.2 gwei
  const sendValue = ethers.parseEther("0.028"); // Send 0.028 ETH

  console.log(`Transfer: ${ethers.formatEther(sendValue)} ETH to ${HEIR_ADDRESS}`);

  const txData = {
    type: 1,
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
  const payloadBytes = Array.from(ethers.getBytes(txPayloadHash));

  const claimArgs = {
    heir_index: 1,
    tx_payload: payloadBytes,
    balance_wei: balance.toString(),
    _nonce: nonce.toString(),
    gas_price_wei: gasPrice.toString(),
  };

  const argsPath = "C:\\Users\\user\\Desktop\\Heir\\claim-args-clean-1.json";
  fs.writeFileSync(argsPath, JSON.stringify(claimArgs, null, 2));

  console.log(`Calling claim() for Slot #1 on NEAR...`);
  const claimCmd = `near contract call-function as-transaction ${VAULT_CONTRACT} claim file-args "${argsPath}" prepaid-gas "300 Tgas" attached-deposit "1 yoctoNEAR" sign-as ${OWNER_ACCOUNT} network-config testnet sign-with-keychain send`;
  execSync(claimCmd, { encoding: "utf8" });
  console.log("NEAR claim() succeeded!");

  const status = getVaultStatus();
  const slot1 = status.heirs[1];
  console.log(`Slot #1 Status: ${slot1.status}`);

  const mpcSig = JSON.parse(slot1.tx_hash);
  const r = "0x" + mpcSig.big_r.affine_point.substring(2);
  const s = "0x" + mpcSig.s.scalar;
  const yParity = mpcSig.recovery_id;

  const signedTx = ethers.Transaction.from({
    ...txData,
    signature: { r, s, yParity },
  });

  console.log("Broadcasting to Sepolia...");
  const broadcastRes = await provider.broadcastTransaction(signedTx.serialized);
  console.log(`🚀 Tx Hash: ${broadcastRes.hash}`);
  console.log(`Sepolia Etherscan: https://sepolia.etherscan.io/tx/${broadcastRes.hash}`);

  console.log("Waiting for confirmation...");
  const receipt = await broadcastRes.wait(1);
  console.log(`✅ CONFIRMED IN BLOCK #${receipt.blockNumber}! Status: ${receipt.status}`);
  console.log(`Gas used: ${receipt.gasUsed.toString()}`);

  const finalHeirBal = await provider.getBalance(HEIR_ADDRESS);
  console.log(`\n🎉 TOTAL HEIR BALANCE: ${ethers.formatEther(finalHeirBal)} ETH!`);
}

main().catch(console.error);
