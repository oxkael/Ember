import { ethers } from "ethers";

const SEPOLIA_RPC = "https://ethereum-sepolia-rpc.publicnode.com";
const provider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

async function broadcast() {
  const r = "0x105e2f4a0ddcd57d4f384b30a7b581f061643ea1d3780d65a221e0bff146d123";
  const s = "0x474948668c93ee798ee6209ee79add84c8e357e37a23d8421799837b0e8b0a44";
  const chainId = 11155111;
  const yParity = 1;

  const txData = {
    type: 1, // EIP-2930
    to: "0x849A9218b6daC5bad1b2411d1708b644f54ceA62",
    value: 24999999999307000n, // ~0.024999 ETH
    nonce: 0,
    gasLimit: 21000n,
    gasPrice: 66n,
    chainId: chainId,
    data: "0x",
    signature: { r, s, yParity }
  };

  const tx = ethers.Transaction.from(txData);
  console.log("Constructed Transaction:");
  console.log("- From:", tx.from);
  console.log("- To:", tx.to);
  console.log("- Value:", ethers.formatEther(tx.value), "ETH");
  console.log("- Nonce:", tx.nonce);
  console.log("- Gas Price:", tx.gasPrice.toString(), "wei");
  console.log("- Serialized Raw Tx:", tx.serialized);

  console.log("\nBroadcasting transaction to Sepolia testnet...");
  try {
    const txResponse = await provider.broadcastTransaction(tx.serialized);
    console.log("\n🚀 Transaction Broadcasted Successfully!");
    console.log("Tx Hash:", txResponse.hash);
    console.log(`Sepolia Etherscan: https://sepolia.etherscan.io/tx/${txResponse.hash}`);
    
    console.log("\nWaiting for block confirmation...");
    const receipt = await txResponse.wait(1);
    console.log(`✅ Confirmed in block #${receipt.blockNumber}!`);
    console.log(`Gas used: ${receipt.gasUsed.toString()}`);

    const vaultBal = await provider.getBalance("0xDAe49575b025e1f21b3238605fE1887c97da210a");
    const heirBal = await provider.getBalance("0x849A9218b6daC5bad1b2411d1708b644f54ceA62");
    console.log(`\nRemaining Vault Balance: ${ethers.formatEther(vaultBal)} ETH`);
    console.log(`New Heir Balance:         ${ethers.formatEther(heirBal)} ETH`);
  } catch (err) {
    console.error("Broadcast error:", err);
  }
}

broadcast();
