const SEPOLIA_RPC_URL = 'https://ethereum-sepolia-rpc.publicnode.com';
const BITCOIN_TESTNET_API = 'https://blockstream.info/testnet/api';
const REQUEST_TIMEOUT_MS = 12_000;

async function fetchWithTimeout(url, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    return await fetch(url, { ...options, signal: controller.signal });
  } finally {
    clearTimeout(timeout);
  }
}

async function postRpc(method, params) {
  const response = await fetchWithTimeout(SEPOLIA_RPC_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  if (!response.ok) throw new Error(`Sepolia balance request failed (${response.status}).`);
  const result = await response.json();
  if (result.error) throw new Error(result.error.message || 'Sepolia RPC returned an error.');
  return result.result;
}

export async function readEthereumBalance(address) {
  if (!/^0x[\da-fA-F]{40}$/.test(address || '')) throw new Error('The Ethereum deposit address is invalid.');
  const chainId = await postRpc('eth_chainId', []);
  if (chainId !== '0xaa36a7') throw new Error('The balance service returned a network other than Sepolia.');
  const balanceWei = await postRpc('eth_getBalance', [address, 'latest']);
  if (!/^0x[\da-fA-F]+$/.test(balanceWei || '')) throw new Error('The balance service returned an invalid Ethereum balance.');
  return BigInt(balanceWei).toString();
}

export async function readBitcoinBalance(address) {
  if (!/^tb1q[\da-z]{38,}$/i.test(address || '')) throw new Error('The Bitcoin Testnet deposit address is invalid.');
  const response = await fetchWithTimeout(`${BITCOIN_TESTNET_API}/address/${encodeURIComponent(address)}`, {
    headers: { Accept: 'application/json' },
  });
  if (!response.ok) throw new Error(`Bitcoin Testnet balance request failed (${response.status}).`);
  const result = await response.json();
  const stats = [result.chain_stats, result.mempool_stats];
  const totalSats = stats.reduce((total, entry) => {
    if (!entry) return total;
    return total + BigInt(entry.funded_txo_sum || 0) - BigInt(entry.spent_txo_sum || 0);
  }, 0n);
  return (totalSats > 0n ? totalSats : 0n).toString();
}
