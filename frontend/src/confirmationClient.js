const ETH_RPC = 'https://ethereum-sepolia-rpc.publicnode.com';
const ETH_BLOCKSCOUT = 'https://eth-sepolia.blockscout.com/api/v2/transactions';
const BTC_BLOCKSTREAM = 'https://blockstream.info/testnet/api';
const BTC_MEMPOOL = 'https://mempool.space/testnet/api';
const TIMEOUT_MS = 12_000;

async function request(url, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const response = await fetch(url, { ...options, signal: controller.signal });
    if (!response.ok) throw new Error(`Service returned ${response.status}.`);
    return response;
  } finally {
    clearTimeout(timeout);
  }
}

async function checkEthRpc(txHash) {
  const response = await request(ETH_RPC, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_getTransactionReceipt', params: [txHash] }),
  });
  const body = await response.json();
  if (body.error) throw new Error(body.error.message || 'Ethereum RPC lookup failed.');
  if (!body.result) return { state: 'Not found or pending', asOf: Date.now() };
  const blockNumber = Number(BigInt(body.result.blockNumber));
  const latestResponse = await request(ETH_RPC, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'eth_blockNumber', params: [] }),
  });
  const latestBody = await latestResponse.json();
  if (latestBody.error || !latestBody.result) throw new Error('Could not read the latest Sepolia block.');
  const confirmations = Math.max(0, Number(BigInt(latestBody.result) - BigInt(blockNumber) + 1n));
  return { state: body.result.status === '0x1' ? 'Confirmed' : 'Failed on-chain', blockNumber, confirmations, asOf: Date.now() };
}

async function checkBlockscout(txHash) {
  const response = await request(`${ETH_BLOCKSCOUT}/${encodeURIComponent(txHash)}`);
  const tx = await response.json();
  if (!tx || typeof tx !== 'object') throw new Error('Blockscout returned an unreadable response.');
  const status = String(tx.status || '').toLowerCase();
  const blockNumber = tx.block_number == null ? null : Number(tx.block_number);
  if (status === 'error' || status === 'reverted') return { state: 'Failed on-chain', blockNumber, asOf: Date.now() };
  if (status === 'ok' || blockNumber != null) return { state: 'Confirmed', blockNumber, asOf: Date.now() };
  return { state: 'Pending', blockNumber: null, asOf: Date.now() };
}

async function checkBitcoin(baseUrl, txid) {
  const response = await request(`${baseUrl}/tx/${encodeURIComponent(txid)}/status`);
  const body = await response.json();
  return {
    state: body.confirmed ? 'Confirmed' : 'In mempool or unconfirmed',
    blockNumber: body.block_height ?? null,
    asOf: Date.now(),
  };
}

export async function checkReportedTransaction(chain, txHash) {
  const isBitcoin = chain === 'Bitcoin';
  const valid = isBitcoin ? /^[\da-f]{64}$/i.test(txHash || '') : /^0x[\da-f]{64}$/i.test(txHash || '');
  if (!valid) throw new Error('The reported transaction hash has an invalid format.');
  const checks = isBitcoin
    ? [
      ['Blockstream', () => checkBitcoin(BTC_BLOCKSTREAM, txHash)],
      ['Mempool.space', () => checkBitcoin(BTC_MEMPOOL, txHash)],
    ]
    : [
      ['Sepolia RPC', () => checkEthRpc(txHash)],
      ['Blockscout', () => checkBlockscout(txHash)],
    ];
  const results = await Promise.allSettled(checks.map(([, run]) => run()));
  return checks.map(([source], index) => ({
    source,
    ...(results[index].status === 'fulfilled'
      ? results[index].value
      : { state: 'Could not check', error: results[index].reason?.message || 'Lookup failed.', asOf: Date.now() }),
  }));
}
