import { ethers } from "ethers";
import { execSync } from "child_process";
import fs from "fs";

// Configuration
const CONFIG = {
  vaultContract: "vault-heirvault01.testnet",
  ownerAccount: "owner-heirvault01.testnet",
  mpcContract: "v1.signer-prod.testnet",
  derivationPath: "ethereum-1",
  vaultEthAddress: "0xDAe49575b025e1f21b3238605fE1887c97da210a",
  sepoliaRpc: "https://ethereum-sepolia-rpc.publicnode.com",
  chainId: 11155111, // Sepolia
  gasLimit: 21000n,
  maxFeeCapWei: 20000000000000000n, // 0.02 ETH
};

const provider = new ethers.JsonRpcProvider(CONFIG.sepoliaRpc);

export async function getVaultStatus() {
  const cmd = `near contract call-function as-read-only ${CONFIG.vaultContract} status file-args C:\\Users\\user\\Desktop\\Heir\\empty.json network-config testnet now`;
  const output = execSync(cmd, { encoding: "utf8" });
  
  // Parse JSON response between braces
  const startIdx = output.indexOf("{\n  \"current_timestamp_ns\"");
  const endIdx = output.lastIndexOf("}") + 1;
  if (startIdx === -1 || endIdx === 0) {
    throw new Error("Could not parse status JSON from CLI output:\n" + output);
  }
  return JSON.parse(output.substring(startIdx, endIdx));
}

export async function pingVault() {
  console.log(`Pinging vault contract from owner ${CONFIG.ownerAccount}...`);
  const cmd = `near contract call-function as-transaction ${CONFIG.vaultContract} ping file-args C:\\Users\\user\\Desktop\\Heir\\empty.json prepaid-gas '100.0 Tgas' attached-deposit '0 NEAR' sign-as ${CONFIG.ownerAccount} network-config testnet sign-with-keychain send`;
  const output = execSync(cmd, { encoding: "utf8" });
  console.log("Ping successful!");
  return output;
}

export async function runRelayer() {
  console.log("=== NEAR INHERITANCE RELAYER ===");
  console.log(`Vault Contract: ${CONFIG.vaultContract}`);
  console.log(`Vault EVM Address: ${CONFIG.vaultEthAddress}`);

  // 1. Check Vault status on NEAR
  const status = await getVaultStatus();
  console.log(`\nVault State: ${status.state}`);
  console.log(`Seconds since heartbeat: ${status.seconds_since_heartbeat}s`);
  console.log(`Seconds until claimable: ${status.seconds_until_claimable}s`);

  // 2. Fetch Sepolia on-chain data
  const balance = await provider.getBalance(CONFIG.vaultEthAddress);
  const nonce = await provider.getTransactionCount(CONFIG.vaultEthAddress, "pending");
  const feeData = await provider.getFeeData();
  const gasPrice = feeData.gasPrice || 1000000000n; // default 1 gwei if not returned

  console.log(`\nSepolia Vault Balance: ${ethers.formatEther(balance)} ETH (${balance.toString()} wei)`);
  console.log(`Current Nonce: ${nonce}`);
  console.log(`Gas Price: ${ethers.formatUnits(gasPrice, "gwei")} gwei (${gasPrice.toString()} wei)`);

  const estFee = CONFIG.gasLimit * gasPrice;
  console.log(`Estimated Transfer Fee: ${ethers.formatEther(estFee)} ETH`);

  if (status.state !== "Claimable") {
    console.log(`\n⚠️  Vault is currently in "${status.state}" state. Claiming is only allowed in "Claimable" state.`);
    return status;
  }

  // Find next unsent heir
  const nextSlotIdx = status.heirs.findIndex(h => h.status === "Unsent");
  if (nextSlotIdx === -1) {
    console.log("\nAll heir distribution slots have already been processed!");
    return status;
  }

  const heirSlot = status.heirs[nextSlotIdx];
  console.log(`\nProcessing Heir Slot #${nextSlotIdx}:`);
  console.log(`- NEAR Account: ${heirSlot.heir.near_account_id}`);
  console.log(`- Sepolia Address: ${heirSlot.heir.ethereum_address}`);
  console.log(`- Share: ${heirSlot.heir.share_basis_points / 100}%`);

  // Calculate heir transfer value
  // value = (total_balance - fee) * share / 10000
  let sendValue = 0n;
  if (balance > estFee) {
    const netPool = balance - estFee;
    sendValue = (netPool * BigInt(heirSlot.heir.share_basis_points)) / 10000n;
  }

  // Build unsigned legacy/EIP-155 Ethereum transaction
  const tx = {
    to: heirSlot.heir.ethereum_address,
    value: sendValue,
    nonce: nonce,
    gasLimit: CONFIG.gasLimit,
    gasPrice: gasPrice,
    chainId: CONFIG.chainId,
    data: "0x",
  };

  const unsignedSerialized = ethers.Transaction.from(tx).unsignedSerialized;
  const txPayloadHash = ethers.keccak256(unsignedSerialized);
  console.log(`Unsigned Tx Hash (Payload for MPC): ${txPayloadHash}`);

  // Convert payload hex to 32 byte array
  const payloadBytes = Array.from(ethers.getBytes(txPayloadHash));

  // Write claim args to file
  const claimArgs = {
    heir_index: nextSlotIdx,
    tx_payload: payloadBytes,
    balance_wei: balance.toString(),
    _nonce: nonce.toString(),
    gas_price_wei: gasPrice.toString(),
  };

  const claimArgsPath = "C:\\Users\\user\\Desktop\\Heir\\claim-args.json";
  fs.writeFileSync(claimArgsPath, JSON.stringify(claimArgs, null, 2));

  console.log(`\nCalling claim() on ${CONFIG.vaultContract} for heir slot #${nextSlotIdx}...`);
  try {
    const claimCmd = `near contract call-function as-transaction ${CONFIG.vaultContract} claim file-args "${claimArgsPath}" prepaid-gas '300.0 Tgas' attached-deposit '1 yoctoNEAR' sign-as ${CONFIG.ownerAccount} network-config testnet sign-with-keychain send`;
    const claimOutput = execSync(claimCmd, { encoding: "utf8" });
    console.log("Claim call submitted to NEAR!");
    console.log(claimOutput);
  } catch (err) {
    console.error("Claim error:", err.message);
  }

  return await getVaultStatus();
}

// Execute if run directly
if (process.argv[1]?.endsWith("relayer.mjs")) {
  runRelayer().catch(console.error);
}
