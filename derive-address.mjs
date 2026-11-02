import { chainAdapters, contracts } from "chainsig.js";
import { createPublicClient, http } from "viem";

const MPC_CONTRACT = "v1.signer-prod.testnet";
const NETWORK_ID = "testnet";
// Test working public Sepolia RPCs
const SEPOLIA_RPC = "https://ethereum-sepolia-rpc.publicnode.com";

const signetContract = new contracts.ChainSignatureContract({
  networkId: NETWORK_ID,
  contractId: MPC_CONTRACT,
});

const publicClient = createPublicClient({
  transport: http(SEPOLIA_RPC),
});

const evm = new chainAdapters.evm.EVM({
  publicClient,
  contract: signetContract,
});

async function testDerivation() {
  const accounts = [
    "owner-heirvault01.testnet",
    "vault-heirvault01.testnet"
  ];
  const path = "ethereum-1";

  for (const account of accounts) {
    console.log(`\n=== Testing Account: ${account} ===`);
    const res1 = await evm.deriveAddressAndPublicKey(account, path);
    const res2 = await evm.deriveAddressAndPublicKey(account, path);

    console.log(`Derived Address: ${res1.address}`);
    console.log(`Match? ${res1.address.toLowerCase() === res2.address.toLowerCase() ? "SUCCESS (Deterministic)" : "FAILED"}`);

    try {
      const balance = await evm.getBalance(res1.address);
      console.log(`Sepolia Balance: ${balance.balance} wei (decimals: ${balance.decimals})`);
    } catch (err) {
      console.log(`Could not fetch balance: ${err.message}`);
    }
  }
}

testDerivation().catch(console.error);
