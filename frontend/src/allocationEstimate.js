export function shareBasisPoints(percent) {
  if (percent == null || String(percent).trim() === "") return null;
  const value = Number(percent);
  if (!Number.isFinite(value) || value < 0 || value > 100) return null;
  return BigInt(Math.round(value * 100));
}

export function estimateShareAmount(balanceBaseUnits, basisPoints) {
  if (balanceBaseUnits == null || basisPoints == null || !/^\d+$/.test(String(balanceBaseUnits))) return null;
  const share = BigInt(basisPoints);
  if (share < 0n || share > 10000n) return null;
  return (BigInt(balanceBaseUnits) * share / 10000n).toString();
}

export function formatTokenAmount(baseUnits, chain) {
  if (baseUnits == null) return null;
  const decimals = chain === "Bitcoin" ? 8 : 18;
  const visibleDecimals = chain === "Bitcoin" ? 8 : 6;
  const symbol = chain === "Bitcoin" ? "BTC" : "ETH";
  const value = BigInt(baseUnits);
  const divisor = 10n ** BigInt(decimals);
  const fraction = (value % divisor).toString().padStart(decimals, "0").slice(0, visibleDecimals).replace(/0+$/, "");
  return `${value / divisor}${fraction ? `.${fraction}` : ""} ${symbol}`;
}
