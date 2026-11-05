use near_sdk::json_types::{U128, U64};
use near_sdk::store::{LookupMap, Vector};
use near_sdk::{env, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError};

/// 250 Tgas attached to MPC sign call
pub const MPC_SIGN_GAS: Gas = Gas::from_tgas(250);
/// 30 Tgas reserved for callback
pub const CALLBACK_GAS: Gas = Gas::from_tgas(30);

/// 1 yoctoNEAR minimum attached deposit for MPC sign call
pub const MPC_SIGN_DEPOSIT: NearToken = NearToken::from_yoctonear(1);

/// Absolute fee cap: 0.002 ETH (2,000,000,000,000,000 Wei)
pub const ABSOLUTE_FEE_CAP_WEI: u128 = 2_000_000_000_000_000;

/// Default gas limit: 250,000 units
pub const DEFAULT_GAS_LIMIT: u64 = 250_000;

/// Fixed Chain ID for Sepolia: 11155111
pub const SEPOLIA_CHAIN_ID: u64 = 11155111;

/// Retry timeout: 60 seconds (in nanoseconds)
pub const RETRY_TIMEOUT_NS: u64 = 60_000_000_000;
/// At most one dedicated keeper is explicitly configured by the owner; an empty list leaves manual claims available.
pub const MAX_KEEPERS: usize = 1;
/// Maximum number of owner records returned by one registry page.
pub const MAX_OWNER_PAGE_SIZE: u32 = 100;
/// Maximum configured heartbeat and grace interval (one year).
pub const MAX_INTERVAL_SEC: u64 = 365 * 24 * 60 * 60;
/// The plan-change review window: fourteen full days in nanoseconds.
pub const PLAN_CHANGE_DELAY_NS: u64 = 14 * 24 * 60 * 60 * 1_000_000_000;

/// Bitcoin P2WPKH Sequence: 0xfffffffd enables BIP-125 Opt-In Replace-By-Fee (RBF)
pub const BTC_P2WPKH_SEQUENCE: u32 = 0xfffffffd;

/// Max Bitcoin fee rate: 200 sat/vB
pub const MAX_BTC_FEE_RATE_SAT_VB: u64 = 200;

/// Legacy test fixture only; production setup must provide the hash derived for its owner path.
#[cfg(test)]
pub const BTC_SENDER_PUBKEY_HASH: &str = "77ed9969481823854b8c850b4596a16030bb3480";

/// Virtual bytes calculation for a 1-input (P2WPKH) native SegWit Bitcoin transaction
/// with variable number of P2WPKH outputs (1 per heir):
/// - 11 vB transaction overhead
/// - 68 vB P2WPKH input
/// - 31 vB per P2WPKH output
pub fn calculate_btc_vbytes(num_heirs: usize) -> u64 {
    11 + 68 + (31 * num_heirs as u64)
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VaultState {
    Uninitialized,
    Active,
    Warning,
    Grace,
    Claimable,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SlotStatus {
    #[default]
    Unsent,
    Pending,
    Signed,
    Done,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WithdrawalChain {
    Ethereum,
    Bitcoin,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WithdrawalStatus {
    Pending,
    Signed,
    Done,
    Failed,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct WithdrawalRecordView {
    pub timestamp_ns: U64,
    pub chain: WithdrawalChain,
    pub amount: U128,
    pub destination: String,
    pub status: WithdrawalStatus,
    pub tx_hash: Option<String>,
    pub reported_tx_hash: Option<String>,
    pub nonce: Option<U64>,
    pub txid: Option<String>,
    pub vout: Option<u32>,
    pub fee_rate_sat_vb: Option<U64>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct PendingPlanChangeView {
    pub effective_at_ns: U64,
    pub heirs: Vec<HeirConfig>,
    pub keepers: Vec<AccountId>,
    pub owner_eth_address: Option<String>,
    pub owner_btc_witness_program: Option<String>,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug)]
pub struct HeirConfig {
    pub near_account_id: AccountId,
    pub ethereum_address: String,
    #[serde(default)]
    pub bitcoin_witness_program: Option<String>,
    pub share_basis_points: u32, // 10000 = 100%, 5000 = 50%
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug, Default)]
pub struct BtcSlot {
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
    pub reported_tx_hash: Option<String>,
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: u64,
    pub last_txid: Option<String>,
    pub last_vout: Option<u32>,
    pub last_fee_rate_sat_vb: Option<u64>,
    pub pending_txid: Option<String>,
    pub pending_vout: Option<u32>,
    pub pending_fee_rate_sat_vb: Option<u64>,
    pub confirmations: Vec<AccountId>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct BtcSlotView {
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
    pub reported_tx_hash: Option<String>,
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: U64,
    pub last_txid: Option<String>,
    pub last_vout: Option<u32>,
    pub last_fee_rate_sat_vb: Option<U64>,
    pub pending_txid: Option<String>,
    pub pending_vout: Option<u32>,
    pub pending_fee_rate_sat_vb: Option<U64>,
    pub confirmations: Vec<AccountId>,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug)]
pub struct HeirSlot {
    pub heir: HeirConfig,
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
    pub reported_tx_hash: Option<String>,
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: u64,
    pub last_nonce: u64,
    pub last_gas_price_wei: U128,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct VaultStatusView {
    pub state: VaultState,
    pub owner_id: AccountId,
    pub last_heartbeat_timestamp_ns: U64,
    pub current_timestamp_ns: U64,
    pub heartbeat_interval_sec: U64,
    pub grace_period_sec: U64,
    pub warning_threshold_sec: U64,
    pub seconds_since_heartbeat: U64,
    pub seconds_until_grace: U64,
    pub seconds_until_claimable: U64,
    pub mpc_contract: AccountId,
    pub derivation_path: String,
    pub btc_derivation_path: String,
    pub gas_limit: U64,
    pub snapshot_balance_wei: Option<U128>,
    pub heirs: Vec<HeirSlotView>,
    pub btc_slot: BtcSlotView,
    pub btc_sender_pubkey_hash: String,
    pub keepers: Vec<AccountId>,
    pub near_signing_reserve_yocto: U128,
    pub signing_deposit_yocto: U128,
    pub eth_outstanding_nonce: Option<U64>,
    pub eth_outstanding_started_at_ns: Option<U64>,
    pub owner_eth_address: Option<String>,
    pub owner_btc_witness_program: Option<String>,
    pub pending_plan_change: Option<PendingPlanChangeView>,
    pub withdrawal_records: Vec<WithdrawalRecordView>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct HeirSlotView {
    pub heir: HeirConfig,
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
    pub reported_tx_hash: Option<String>,
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: U64,
    pub last_nonce: U64,
    pub last_gas_price_wei: U128,
}

// Payload structure expected by MPC signer contract v1.signer-prod.testnet
#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct SignRequest {
    pub payload: [u8; 32],
    pub path: String,
    pub key_version: u32,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct VaultContract {
    pub mpc_contract: AccountId,
    vaults: LookupMap<AccountId, VaultRecord>,
    owner_ids: Vector<AccountId>,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
struct VaultRecord {
    owner_id: AccountId,
    derivation_path: String,
    btc_derivation_path: String,
    pub last_heartbeat_timestamp_ns: u64,
    pub heartbeat_interval_ns: u64,
    pub grace_period_ns: u64,
    pub is_configured: bool,
    pub gas_limit: u64,
    pub snapshot_balance_wei: Option<u128>,
    pub heirs: Vec<HeirSlot>,
    pub btc_slot: BtcSlot,
    pub btc_sender_pubkey_hash: String,
    pub keepers: Vec<AccountId>,
    pub eth_outstanding: Option<EthOutstanding>,
    pub last_done_eth_nonce: Option<u64>,
    pub owner_eth_address: Option<String>,
    pub owner_btc_witness_program: Option<String>,
    pub pending_plan_change: Option<PendingPlanChange>,
    pub withdrawals: Vec<WithdrawalRecord>,
    pub btc_withdrawal_outstanding_index: Option<u32>,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
struct PendingPlanChange {
    heirs: Vec<HeirSlot>,
    keepers: Vec<AccountId>,
    owner_eth_address: Option<String>,
    owner_btc_witness_program: Option<String>,
    effective_at_ns: u64,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
struct WithdrawalRecord {
    timestamp_ns: u64,
    chain: WithdrawalChain,
    amount: u128,
    destination: String,
    status: WithdrawalStatus,
    tx_hash: Option<String>,
    reported_tx_hash: Option<String>,
    nonce: Option<u64>,
    gas_price_wei: Option<u128>,
    txid: Option<String>,
    vout: Option<u32>,
    value_sats: Option<u64>,
    fee_rate_sat_vb: Option<u64>,
}

#[near(serializers = [borsh])]
#[derive(Clone, Debug)]
struct EthOutstanding {
    slot_index: u32,
    nonce: u64,
    started_at_ns: u64,
    withdrawal_index: Option<u32>,
}

// RLP Encoding & EIP-2930 Builder
pub fn encode_u64_rlp(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80];
    }
    let bytes = val.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    let trimmed = &bytes[start..];
    if trimmed.len() == 1 && trimmed[0] < 0x80 {
        vec![trimmed[0]]
    } else {
        let mut out = Vec::with_capacity(1 + trimmed.len());
        out.push(0x80 + trimmed.len() as u8);
        out.extend_from_slice(trimmed);
        out
    }
}

pub fn encode_u128_rlp(val: u128) -> Vec<u8> {
    if val == 0 {
        return vec![0x80];
    }
    let bytes = val.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    let trimmed = &bytes[start..];
    if trimmed.len() == 1 && trimmed[0] < 0x80 {
        vec![trimmed[0]]
    } else {
        let mut out = Vec::with_capacity(1 + trimmed.len());
        out.push(0x80 + trimmed.len() as u8);
        out.extend_from_slice(trimmed);
        out
    }
}

pub fn encode_bytes_rlp(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        vec![data[0]]
    } else if data.len() <= 55 {
        let mut out = Vec::with_capacity(1 + data.len());
        out.push(0x80 + data.len() as u8);
        out.extend_from_slice(data);
        out
    } else {
        let len_bytes = data.len().to_be_bytes();
        let start = len_bytes.iter().position(|&b| b != 0).unwrap_or(len_bytes.len());
        let trimmed_len = &len_bytes[start..];
        let mut out = Vec::with_capacity(1 + trimmed_len.len() + data.len());
        out.push(0xb7 + trimmed_len.len() as u8);
        out.extend_from_slice(trimmed_len);
        out.extend_from_slice(data);
        out
    }
}

pub fn encode_list_rlp(payload: &[u8]) -> Vec<u8> {
    if payload.len() <= 55 {
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(0xc0 + payload.len() as u8);
        out.extend_from_slice(payload);
        out
    } else {
        let len_bytes = payload.len().to_be_bytes();
        let start = len_bytes.iter().position(|&b| b != 0).unwrap_or(len_bytes.len());
        let trimmed_len = &len_bytes[start..];
        let mut out = Vec::with_capacity(1 + trimmed_len.len() + payload.len());
        out.push(0xf7 + trimmed_len.len() as u8);
        out.extend_from_slice(trimmed_len);
        out.extend_from_slice(payload);
        out
    }
}

pub fn hex_to_bytes20(hex_str: &str) -> [u8; 20] {
    let clean = if hex_str.starts_with("0x") || hex_str.starts_with("0X") {
        &hex_str[2..]
    } else {
        hex_str
    };
    require!(clean.len() == 40, "Invalid 20-byte hex address length");
    let mut out = [0u8; 20];
    for i in 0..20 {
        out[i] = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16)
            .expect("Invalid hex character in address");
    }
    out
}

pub fn hex_to_bytes32(hex_str: &str) -> [u8; 32] {
    let clean = if hex_str.starts_with("0x") || hex_str.starts_with("0X") {
        &hex_str[2..]
    } else {
        hex_str
    };
    require!(clean.len() == 64, "Invalid 32-byte hex string length");
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16)
            .expect("Invalid hex character in 32-byte string");
    }
    out
}

/// Pure Rust implementation of SHA-256
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_val = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    let mut result = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        result[i * 4..(i + 1) * 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

/// Pure Rust double-SHA256 (SHA-256(SHA-256(data)))
pub fn double_sha256(data: &[u8]) -> [u8; 32] {
    sha256(&sha256(data))
}

#[derive(Clone, Debug)]
pub struct BtcOutput {
    pub value_sats: u64,
    pub witness_program: [u8; 20],
}

/// Pure Rust function that computes the BIP-143 sighash for a P2WPKH input with SIGHASH_ALL (0x01).
pub fn compute_bip143_p2wpkh_sighash(
    version: u32,
    txid: [u8; 32], // in big-endian hex display order (reversed internally to little-endian for wire format)
    vout: u32,
    sender_pubkey_hash: [u8; 20],
    input_value_sats: u64,
    sequence: u32,
    outputs: &[BtcOutput],
    locktime: u32,
) -> [u8; 32] {
    // 1. nVersion (4 bytes LE)
    let version_bytes = version.to_le_bytes();

    // 2. hashPrevouts: double_sha256 of all input outpoints (36 bytes each)
    let mut outpoint = Vec::with_capacity(36);
    let mut txid_le = txid;
    txid_le.reverse();
    outpoint.extend_from_slice(&txid_le);
    outpoint.extend_from_slice(&vout.to_le_bytes());
    let hash_prevouts = double_sha256(&outpoint);

    // 3. hashSequence: double_sha256 of all input sequences (4 bytes each)
    let hash_sequence = double_sha256(&sequence.to_le_bytes());

    // 4. outpoint (assembled above)

    // 5. scriptCode: 0x19, 0x76, 0xa9, 0x14, <20-byte pubkey hash>, 0x88, 0xac
    let mut script_code = Vec::with_capacity(26);
    script_code.extend_from_slice(&[0x19, 0x76, 0xa9, 0x14]);
    script_code.extend_from_slice(&sender_pubkey_hash);
    script_code.extend_from_slice(&[0x88, 0xac]);

    // 6. amount: 8 bytes LE
    let amount_bytes = input_value_sats.to_le_bytes();

    // 7. nSequence: 4 bytes LE
    let sequence_bytes = sequence.to_le_bytes();

    // 8. hashOutputs: double_sha256 of all serialized outputs
    let mut outputs_bytes = Vec::new();
    for out in outputs {
        outputs_bytes.extend_from_slice(&out.value_sats.to_le_bytes());
        // Native SegWit P2WPKH scriptPubKey: 0x16, 0x00, 0x14, <20-byte witness program>
        outputs_bytes.extend_from_slice(&[0x16, 0x00, 0x14]);
        outputs_bytes.extend_from_slice(&out.witness_program);
    }
    let hash_outputs = double_sha256(&outputs_bytes);

    // 9. nLockTime: 4 bytes LE
    let locktime_bytes = locktime.to_le_bytes();

    // 10. sighash type: 4 bytes LE (1 for SIGHASH_ALL)
    let sighash_type = 1u32.to_le_bytes();

    // Assemble BIP-143 preimage
    let mut preimage = Vec::with_capacity(4 + 32 + 32 + 36 + 26 + 8 + 4 + 32 + 4 + 4);
    preimage.extend_from_slice(&version_bytes);
    preimage.extend_from_slice(&hash_prevouts);
    preimage.extend_from_slice(&hash_sequence);
    preimage.extend_from_slice(&outpoint);
    preimage.extend_from_slice(&script_code);
    preimage.extend_from_slice(&amount_bytes);
    preimage.extend_from_slice(&sequence_bytes);
    preimage.extend_from_slice(&hash_outputs);
    preimage.extend_from_slice(&locktime_bytes);
    preimage.extend_from_slice(&sighash_type);

    double_sha256(&preimage)
}

/// Pure function to calculate Bitcoin outputs for heirs, enforcing fee caps, dust threshold,
/// and allocating remainder to the last heir.
pub fn calculate_btc_outputs(
    input_value_sats: u64,
    fee_rate_sat_vb: u64,
    heirs: &[HeirConfig],
) -> Result<Vec<BtcOutput>, String> {
    if fee_rate_sat_vb == 0 {
        return Err("Fee rate must be > 0".to_string());
    }
    if fee_rate_sat_vb > MAX_BTC_FEE_RATE_SAT_VB {
        return Err(format!(
            "Fee rate {} sat/vB exceeds maximum cap of {} sat/vB",
            fee_rate_sat_vb, MAX_BTC_FEE_RATE_SAT_VB
        ));
    }

    if heirs.is_empty() {
        return Err("Heirs list cannot be empty".to_string());
    }

    let vbytes = calculate_btc_vbytes(heirs.len());
    let fee_sats = vbytes
        .checked_mul(fee_rate_sat_vb)
        .ok_or_else(|| "Fee calculation overflow".to_string())?;

    if input_value_sats <= fee_sats {
        return Err("Input value must exceed fee".to_string());
    }

    let max_allowable_fee = (input_value_sats as u128)
        .checked_mul(5)
        .ok_or_else(|| "Max fee calculation overflow".to_string())?
        / 100;
    if (fee_sats as u128) > max_allowable_fee {
        return Err("Estimated fee exceeds 5% of input value".to_string());
    }

    for heir in heirs {
        if heir.bitcoin_witness_program.is_none() {
            return Err("All heirs must have a Bitcoin witness program configured".to_string());
        }
    }

    let pool_sats = input_value_sats - fee_sats;
    let num_heirs = heirs.len();
    let mut outputs: Vec<BtcOutput> = Vec::with_capacity(num_heirs);
    let mut allocated_sats = 0u64;

    for (i, heir) in heirs.iter().enumerate() {
        let program_str = heir
            .bitcoin_witness_program
            .as_ref()
            .ok_or_else(|| "Missing Bitcoin witness program".to_string())?;
        let program = hex_to_bytes20(program_str);

        let output_value = if i == num_heirs - 1 {
            pool_sats
                .checked_sub(allocated_sats)
                .ok_or_else(|| "Remainder underflow".to_string())?
        } else {
            let val = (pool_sats as u128)
                .checked_mul(heir.share_basis_points as u128)
                .ok_or_else(|| "Output calculation overflow".to_string())?
                / 10000;
            val as u64
        };

        if output_value < 294 {
            return Err(format!(
                "Output value {} sats is below dust threshold (294 sats)",
                output_value
            ));
        }

        allocated_sats = allocated_sats
            .checked_add(output_value)
            .ok_or_else(|| "Allocated sats overflow".to_string())?;

        outputs.push(BtcOutput {
            value_sats: output_value,
            witness_program: program,
        });
    }

    Ok(outputs)
}

pub fn build_eip2930_unsigned_tx(
    chain_id: u64,
    nonce: u64,
    gas_price: u128,
    gas_limit: u64,
    to: [u8; 20],
    value: u128,
) -> Vec<u8> {
    let mut inner = Vec::new();
    inner.extend_from_slice(&encode_u64_rlp(chain_id));
    inner.extend_from_slice(&encode_u64_rlp(nonce));
    inner.extend_from_slice(&encode_u128_rlp(gas_price));
    inner.extend_from_slice(&encode_u64_rlp(gas_limit));
    inner.extend_from_slice(&encode_bytes_rlp(&to));
    inner.extend_from_slice(&encode_u128_rlp(value));
    inner.push(0x80); // empty data
    inner.push(0xc0); // empty access list

    let list_rlp = encode_list_rlp(&inner);
    let mut tx = Vec::with_capacity(1 + list_rlp.len());
    tx.push(0x01); // EIP-2930 prefix
    tx.extend_from_slice(&list_rlp);
    tx
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde", deny_unknown_fields)]
pub struct ClaimArgs {
    pub owner_id: AccountId,
    pub heir_index: u32,
    pub balance_wei: U128,
    pub nonce: U64,
    pub gas_price_wei: U128,
}

fn deserialize_u64_flex<'de, D>(deserializer: D) -> Result<U64, D::Error>
where
    D: near_sdk::serde::Deserializer<'de>,
{
    struct U64FlexVisitor;

    impl<'de> near_sdk::serde::de::Visitor<'de> for U64FlexVisitor {
        type Value = U64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a u64 integer or string")
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(U64(v))
        }

        fn visit_i64<E: near_sdk::serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            if v >= 0 {
                Ok(U64(v as u64))
            } else {
                Err(E::custom("negative integer for u64"))
            }
        }

        fn visit_str<E: near_sdk::serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            v.parse::<u64>().map(U64).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(U64FlexVisitor)
}

#[derive(near_sdk::serde::Serialize, near_sdk::serde::Deserialize, Debug, Clone)]
#[serde(crate = "near_sdk::serde", deny_unknown_fields)]
pub struct ClaimBtcArgs {
    pub owner_id: AccountId,
    pub txid: String,
    pub vout: u32,
    #[serde(deserialize_with = "deserialize_u64_flex")]
    pub value_sats: U64,
    #[serde(deserialize_with = "deserialize_u64_flex")]
    pub fee_rate_sat_vb: U64,
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde", deny_unknown_fields)]
pub struct WithdrawEthArgs {
    pub balance_wei: U128,
    pub nonce: U64,
    pub gas_price_wei: U128,
}

#[derive(near_sdk::serde::Deserialize)]
#[serde(crate = "near_sdk::serde", deny_unknown_fields)]
pub struct WithdrawBtcArgs {
    pub txid: String,
    pub vout: u32,
    #[serde(deserialize_with = "deserialize_u64_flex")]
    pub value_sats: U64,
    #[serde(deserialize_with = "deserialize_u64_flex")]
    pub fee_rate_sat_vb: U64,
}

#[near]
impl VaultContract {
    #[init]
    pub fn new(
        mpc_contract: Option<AccountId>,
    ) -> Self {
        Self {
            mpc_contract: mpc_contract.unwrap_or_else(|| "v1.signer-prod.testnet".parse().unwrap()),
            vaults: LookupMap::new(b"vaults".to_vec()),
            owner_ids: Vector::new(b"owners".to_vec()),
        }
    }

    fn load_vault(&self, owner_id: &AccountId) -> VaultRecord {
        self.vaults
            .get(owner_id)
            .cloned()
            .unwrap_or_else(|| env::panic_str("Vault record not found"))
    }

    fn store_vault(&mut self, owner_id: &AccountId, vault: &VaultRecord) {
        self.vaults.insert(owner_id.clone(), vault.clone());
    }

    fn available_near_balance_yocto(&self) -> u128 {
        let account_balance = env::account_balance().as_yoctonear();
        let locked_balance = env::account_locked_balance().as_yoctonear();
        account_balance.saturating_sub(locked_balance)
    }

    fn assert_signing_reserve(&self) {
        require!(
            self.available_near_balance_yocto() >= MPC_SIGN_DEPOSIT.as_yoctonear(),
            "Contract NEAR balance is too low to pay MPC signing deposit"
        );
    }

    fn apply_matured_plan_change(&mut self, owner_id: &AccountId) -> VaultRecord {
        let mut vault = self.load_vault(owner_id);
        if let Some(pending) = vault.pending_plan_change.clone() {
            let claimable_at = vault
                .last_heartbeat_timestamp_ns
                .saturating_add(vault.heartbeat_interval_ns)
                .saturating_add(vault.grace_period_ns);
            let now = env::block_timestamp();
            if pending.effective_at_ns >= claimable_at {
                // A plan change that would mature at or after Claimable is discarded.
                vault.pending_plan_change = None;
                self.store_vault(owner_id, &vault);
            } else if now >= pending.effective_at_ns {
                // Its effective time was before Claimable, so it is applied even if
                // the first transaction that notices it arrives later.
                vault.heirs = pending.heirs;
                vault.keepers = pending.keepers;
                vault.owner_eth_address = pending.owner_eth_address;
                vault.owner_btc_witness_program = pending.owner_btc_witness_program;
                vault.pending_plan_change = None;
                vault.snapshot_balance_wei = None;
                vault.btc_slot = BtcSlot::default();
                self.store_vault(owner_id, &vault);
            }
        }
        vault
    }

    fn status_for(&self, vault: &VaultRecord) -> VaultStatusView {
        let current_ns = env::block_timestamp();
        let state = vault.calculate_state();
        let elapsed_ns = current_ns.saturating_sub(vault.last_heartbeat_timestamp_ns);

        let warning_threshold_ns = vault.heartbeat_interval_ns * 3 / 4;
        let grace_start_ns = vault.heartbeat_interval_ns;
        let claimable_start_ns = vault.heartbeat_interval_ns + vault.grace_period_ns;

        let seconds_since_heartbeat = elapsed_ns / 1_000_000_000;
        let seconds_until_grace = grace_start_ns.saturating_sub(elapsed_ns) / 1_000_000_000;
        let seconds_until_claimable = claimable_start_ns.saturating_sub(elapsed_ns) / 1_000_000_000;

        VaultStatusView {
            state,
            owner_id: vault.owner_id.clone(),
            last_heartbeat_timestamp_ns: U64(vault.last_heartbeat_timestamp_ns),
            current_timestamp_ns: U64(current_ns),
            heartbeat_interval_sec: U64(vault.heartbeat_interval_ns / 1_000_000_000),
            grace_period_sec: U64(vault.grace_period_ns / 1_000_000_000),
            warning_threshold_sec: U64(warning_threshold_ns / 1_000_000_000),
            seconds_since_heartbeat: U64(seconds_since_heartbeat),
            seconds_until_grace: U64(seconds_until_grace),
            seconds_until_claimable: U64(seconds_until_claimable),
            mpc_contract: self.mpc_contract.clone(),
            derivation_path: vault.derivation_path.clone(),
            btc_derivation_path: vault.btc_derivation_path.clone(),
            gas_limit: U64(vault.gas_limit),
            snapshot_balance_wei: vault.snapshot_balance_wei.map(U128),
            heirs: vault
                .heirs
                .iter()
                .map(|h| HeirSlotView {
                    heir: h.heir.clone(),
                    status: h.status,
                    tx_hash: h.tx_hash.clone(),
                    reported_tx_hash: h.reported_tx_hash.clone(),
                    signed_hash: h.signed_hash.clone(),
                    last_claim_timestamp_ns: U64(h.last_claim_timestamp_ns),
                    last_nonce: U64(h.last_nonce),
                    last_gas_price_wei: h.last_gas_price_wei,
                })
                .collect(),
            btc_slot: BtcSlotView {
                status: vault.btc_slot.status,
                tx_hash: vault.btc_slot.tx_hash.clone(),
                reported_tx_hash: vault.btc_slot.reported_tx_hash.clone(),
                signed_hash: vault.btc_slot.signed_hash.clone(),
                last_claim_timestamp_ns: U64(vault.btc_slot.last_claim_timestamp_ns),
                last_txid: vault.btc_slot.last_txid.clone(),
                last_vout: vault.btc_slot.last_vout,
                last_fee_rate_sat_vb: vault.btc_slot.last_fee_rate_sat_vb.map(U64),
                pending_txid: vault.btc_slot.pending_txid.clone(),
                pending_vout: vault.btc_slot.pending_vout,
                pending_fee_rate_sat_vb: vault.btc_slot.pending_fee_rate_sat_vb.map(U64),
                confirmations: vault.btc_slot.confirmations.clone(),
            },
            btc_sender_pubkey_hash: vault.btc_sender_pubkey_hash.clone(),
            keepers: vault.keepers.clone(),
            near_signing_reserve_yocto: U128(self.available_near_balance_yocto()),
            signing_deposit_yocto: U128(MPC_SIGN_DEPOSIT.as_yoctonear()),
            eth_outstanding_nonce: vault.eth_outstanding.as_ref().map(|pending| U64(pending.nonce)),
            eth_outstanding_started_at_ns: vault.eth_outstanding.as_ref().map(|pending| U64(pending.started_at_ns)),
            owner_eth_address: vault.owner_eth_address.clone(),
            owner_btc_witness_program: vault.owner_btc_witness_program.clone(),
            pending_plan_change: vault.pending_plan_change.as_ref().map(|pending| PendingPlanChangeView {
                effective_at_ns: U64(pending.effective_at_ns),
                heirs: pending.heirs.iter().map(|slot| slot.heir.clone()).collect(),
                keepers: pending.keepers.clone(),
                owner_eth_address: pending.owner_eth_address.clone(),
                owner_btc_witness_program: pending.owner_btc_witness_program.clone(),
            }),
            withdrawal_records: vault.withdrawals.iter().map(|record| WithdrawalRecordView {
                timestamp_ns: U64(record.timestamp_ns),
                chain: record.chain,
                amount: U128(record.amount),
                destination: record.destination.clone(),
                status: record.status,
                tx_hash: record.tx_hash.clone(),
                reported_tx_hash: record.reported_tx_hash.clone(),
                nonce: record.nonce.map(U64),
                txid: record.txid.clone(),
                vout: record.vout,
                fee_rate_sat_vb: record.fee_rate_sat_vb.map(U64),
            }).collect(),
        }
    }

    /// Owner creates or updates only their own vault record.
    pub fn setup(
        &mut self,
        heirs: Vec<HeirConfig>,
        heartbeat_interval_sec: u64,
        grace_period_sec: u64,
        gas_limit: Option<U64>,
        btc_sender_pubkey_hash: String,
        keeper_id: Option<AccountId>,
    ) {
        self.setup_with_keepers(
            heirs,
            heartbeat_interval_sec,
            grace_period_sec,
            gas_limit,
            btc_sender_pubkey_hash,
            keeper_id.into_iter().collect(),
        );
    }

    /// Owner explicitly names at most one dedicated keeper for this vault record.
    pub fn setup_with_keepers(
        &mut self,
        heirs: Vec<HeirConfig>,
        heartbeat_interval_sec: u64,
        grace_period_sec: u64,
        gas_limit: Option<U64>,
        btc_sender_pubkey_hash: String,
        keepers: Vec<AccountId>,
    ) {
        let owner_id = env::predecessor_account_id();
        let eth_path = format!("ember/v1/{owner_id}/ethereum");
        let btc_path = format!("ember/v1/{owner_id}/bitcoin");
        let mut vault = self.vaults.get(&owner_id).cloned().unwrap_or_else(|| VaultRecord {
            owner_id: owner_id.clone(),
            derivation_path: eth_path,
            btc_derivation_path: btc_path,
            last_heartbeat_timestamp_ns: env::block_timestamp(),
            heartbeat_interval_ns: 120_000_000_000,
            grace_period_ns: 60_000_000_000,
            is_configured: false,
            gas_limit: DEFAULT_GAS_LIMIT,
            snapshot_balance_wei: None,
            heirs: Vec::new(),
            btc_slot: BtcSlot::default(),
            btc_sender_pubkey_hash: String::new(),
            keepers: Vec::new(),
            eth_outstanding: None,
            last_done_eth_nonce: None,
            owner_eth_address: None,
            owner_btc_witness_program: None,
            pending_plan_change: None,
            withdrawals: Vec::new(),
            btc_withdrawal_outstanding_index: None,
        });
        require!(!vault.is_configured, "Vault is already configured; use the plan-change method");
        require!(keepers.len() <= MAX_KEEPERS, "Only one keeper may be configured");
        for (index, keeper) in keepers.iter().enumerate() {
            require!(keeper != &owner_id, "Keeper must be separate from the owner");
            require!(!keepers[..index].contains(keeper), "Keeper list cannot contain duplicates");
        }

        let clean_btc_hash = btc_sender_pubkey_hash.strip_prefix("0x")
            .or_else(|| btc_sender_pubkey_hash.strip_prefix("0X"))
            .unwrap_or(&btc_sender_pubkey_hash);
        require!(clean_btc_hash.len() == 40, "Invalid Bitcoin sender witness program length");
        require!(clean_btc_hash.chars().all(|c| c.is_ascii_hexdigit()), "Invalid Bitcoin sender witness program hex");
        let clean_btc_hash = clean_btc_hash.to_ascii_lowercase();
        if vault.is_configured {
            require!(
                vault.btc_sender_pubkey_hash == clean_btc_hash,
                "Bitcoin signing address cannot change after initial setup"
            );
        }

        // Block setup during Grace or Claimable
        if vault.is_configured {
            let current_state = vault.calculate_state();
            require!(
                current_state != VaultState::Grace && current_state != VaultState::Claimable,
                "Setup is blocked during Grace or Claimable state"
            );

            // Block setup once any slot has started (Pending, Signed, Done)
            for slot in &vault.heirs {
                require!(
                    slot.status == SlotStatus::Unsent,
                    "Setup is blocked once any heir slot distribution has started"
                );
            }
            require!(
                vault.btc_slot.status == SlotStatus::Unsent,
                "Setup is blocked once BTC slot distribution has started"
            );
        }

        require!(!heirs.is_empty(), "Heirs list cannot be empty");
        require!(heartbeat_interval_sec > 0, "Heartbeat interval must be > 0");
        require!(grace_period_sec > 0, "Grace period must be > 0");
        require!(heartbeat_interval_sec <= MAX_INTERVAL_SEC, "Heartbeat interval cannot exceed 365 days");
        require!(grace_period_sec <= MAX_INTERVAL_SEC, "Grace period cannot exceed 365 days");

        let total_shares: u32 = heirs.iter().map(|h| h.share_basis_points).sum();
        require!(
            total_shares == 10000,
            "Total heir shares must equal 10000 (100%)"
        );

        for heir in &heirs {
            require!(
                heir.ethereum_address.starts_with("0x") && heir.ethereum_address.len() == 42,
                "Invalid Ethereum address format"
            );
            if let Some(ref btc_wp) = heir.bitcoin_witness_program {
                let clean = if btc_wp.starts_with("0x") || btc_wp.starts_with("0X") {
                    &btc_wp[2..]
                } else {
                    btc_wp.as_str()
                };
                require!(clean.len() == 40, "Invalid Bitcoin 20-byte witness program hex length");
                for c in clean.chars() {
                    require!(c.is_ascii_hexdigit(), "Invalid hex char in Bitcoin witness program");
                }
            }
        }

        vault.heartbeat_interval_ns = heartbeat_interval_sec * 1_000_000_000;
        vault.grace_period_ns = grace_period_sec * 1_000_000_000;
        vault.last_heartbeat_timestamp_ns = env::block_timestamp();
        vault.is_configured = true;
        vault.snapshot_balance_wei = None;
        vault.btc_sender_pubkey_hash = clean_btc_hash;
        if let Some(gl) = gas_limit {
            vault.gas_limit = gl.0;
        }

        vault.heirs = heirs
            .into_iter()
            .map(|heir| HeirSlot {
                heir,
                status: SlotStatus::Unsent,
                tx_hash: None,
                reported_tx_hash: None,
                signed_hash: None,
                last_claim_timestamp_ns: 0,
                last_nonce: 0,
                last_gas_price_wei: U128(0),
            })
            .collect();

        vault.btc_slot = BtcSlot::default();
        vault.keepers = keepers;
        // A configured owner must be discoverable by the shared keeper service.
        self.owner_ids.push(owner_id.clone());
        self.store_vault(&owner_id, &vault);
    }

    /// Propose the owner's external-chain withdrawal destinations. The active addresses stay in use
    /// for fourteen days, including when this is the first address registration after setup.
    pub fn set_withdrawal_destinations(
        &mut self,
        ethereum_address: String,
        bitcoin_witness_program: String,
    ) {
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.calculate_state() != VaultState::Claimable, "Cannot change destinations after Claimable");
        let eth = normalize_eth_address(&ethereum_address);
        let btc = normalize_witness_program(&bitcoin_witness_program);

        let mut pending = vault.pending_plan_change.clone().unwrap_or_else(|| PendingPlanChange {
            heirs: vault.heirs.clone(),
            keepers: vault.keepers.clone(),
            owner_eth_address: vault.owner_eth_address.clone(),
            owner_btc_witness_program: vault.owner_btc_witness_program.clone(),
            effective_at_ns: 0,
        });
        pending.owner_eth_address = Some(eth);
        pending.owner_btc_witness_program = Some(btc);
        pending.effective_at_ns = env::block_timestamp().saturating_add(PLAN_CHANGE_DELAY_NS);
        vault.pending_plan_change = Some(pending);
        self.store_vault(&owner_id, &vault);
    }

    /// Propose a new recipient/share/keeper plan. The active plan remains in force for fourteen days.
    pub fn propose_plan_change(&mut self, heirs: Vec<HeirConfig>, keepers: Vec<AccountId>) {
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.calculate_state() != VaultState::Claimable, "Cannot change the plan after Claimable");
        require!(keepers.len() <= MAX_KEEPERS, "Only one keeper may be configured");
        require!(!keepers.contains(&owner_id), "Keeper must be separate from the owner");
        require!(!has_distribution_started(&vault), "Cannot change the plan after distribution has started");
        validate_heirs(&heirs);

        let mut pending = vault.pending_plan_change.clone().unwrap_or_else(|| PendingPlanChange {
            heirs: vault.heirs.clone(),
            keepers: vault.keepers.clone(),
            owner_eth_address: vault.owner_eth_address.clone(),
            owner_btc_witness_program: vault.owner_btc_witness_program.clone(),
            effective_at_ns: 0,
        });
        pending.heirs = heirs.into_iter().map(new_heir_slot).collect();
        pending.keepers = keepers;
        pending.effective_at_ns = env::block_timestamp().saturating_add(PLAN_CHANGE_DELAY_NS);
        vault.pending_plan_change = Some(pending);
        self.store_vault(&owner_id, &vault);
    }

    /// The owner may cancel a proposed plan or destination change before it becomes effective.
    pub fn cancel_plan_change(&mut self) {
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.pending_plan_change.is_some(), "No pending plan change");
        vault.pending_plan_change = None;
        self.store_vault(&owner_id, &vault);
    }

    /// Anyone may finalize a matured plan change. No scheduler is required; status exposes its deadline.
    pub fn apply_plan_change(&mut self, owner_id: AccountId) {
        let _ = self.apply_matured_plan_change(&owner_id);
    }

    /// Owner can change the check-in and grace periods before Claimable. The timer restarts immediately.
    pub fn update_schedule(&mut self, heartbeat_interval_sec: u64, grace_period_sec: u64) {
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.calculate_state() != VaultState::Claimable, "Cannot change schedule after Claimable");
        require!(heartbeat_interval_sec > 0 && heartbeat_interval_sec <= MAX_INTERVAL_SEC,
            "Heartbeat interval must be between 1 second and 365 days");
        require!(grace_period_sec > 0 && grace_period_sec <= MAX_INTERVAL_SEC,
            "Grace period must be between 1 second and 365 days");
        vault.heartbeat_interval_ns = heartbeat_interval_sec.saturating_mul(1_000_000_000);
        vault.grace_period_ns = grace_period_sec.saturating_mul(1_000_000_000);
        vault.last_heartbeat_timestamp_ns = env::block_timestamp();
        self.store_vault(&owner_id, &vault);
    }

    /// Owner signs a withdrawal to the currently registered Ethereum destination.
    /// The destination cannot be supplied in call arguments and the heartbeat is unchanged.
    pub fn withdraw_eth(&mut self) -> Promise {
        let args: WithdrawEthArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.calculate_state() != VaultState::Claimable, "Withdrawals are disabled after Claimable");
        let destination = vault.owner_eth_address.clone().unwrap_or_else(|| env::panic_str("Ethereum withdrawal destination is not registered"));
        self.assert_signing_reserve();

        let nonce = args.nonce.0;
        match vault.eth_outstanding.as_ref() {
            Some(pending) => {
                require!(pending.slot_index == u32::MAX, "An Ethereum claim is already outstanding");
                require!(pending.nonce == nonce, "Retry must use the outstanding Ethereum nonce");
                require!(env::block_timestamp().saturating_sub(pending.started_at_ns) >= RETRY_TIMEOUT_NS,
                    "Ethereum withdrawal retry is allowed only after 60 seconds");
                let previous_gas = pending.withdrawal_index.and_then(|i| vault.withdrawals.get(i as usize))
                    .and_then(|r| r.gas_price_wei).unwrap_or(0);
                require!(args.gas_price_wei.0 > previous_gas, "Retry must provide a higher gas price");
            }
            None => {
                let expected = vault.last_done_eth_nonce.map(|n| n.saturating_add(1)).unwrap_or(0);
                require!(nonce == expected, "Ethereum nonce must follow the last completed transaction");
            }
        }

        let fee = (vault.gas_limit as u128).checked_mul(args.gas_price_wei.0).expect("Fee calculation overflow");
        require!(fee <= ABSOLUTE_FEE_CAP_WEI, "Estimated fee exceeds absolute fee cap (0.002 ETH)");
        require!(args.balance_wei.0 > fee, "Withdrawal balance is insufficient to cover fee");
        require!(fee <= args.balance_wei.0.checked_mul(5).expect("Fee cap overflow") / 100,
            "Estimated fee exceeds 5% of withdrawal balance");
        let amount = args.balance_wei.0 - fee;
        let destination_bytes = hex_to_bytes20(&destination);
        let unsigned_tx = build_eip2930_unsigned_tx(SEPOLIA_CHAIN_ID, nonce, args.gas_price_wei.0,
            vault.gas_limit, destination_bytes, amount);
        let sighash = env::keccak256_array(&unsigned_tx);
        let now = env::block_timestamp();
        let index = vault.withdrawals.len() as u32;
        vault.withdrawals.push(WithdrawalRecord {
            timestamp_ns: now, chain: WithdrawalChain::Ethereum, amount, destination: destination.clone(),
            status: WithdrawalStatus::Pending, tx_hash: None, reported_tx_hash: None,
            nonce: Some(nonce), gas_price_wei: Some(args.gas_price_wei.0), txid: None, vout: None,
            value_sats: None, fee_rate_sat_vb: None,
        });
        vault.eth_outstanding = Some(EthOutstanding { slot_index: u32::MAX, nonce, started_at_ns: now,
            withdrawal_index: Some(index) });
        let request = SignRequest { payload: sighash, path: vault.derivation_path.clone(), key_version: 0 };
        let mpc = Promise::new(self.mpc_contract.clone()).function_call("sign".to_string(),
            near_sdk::serde_json::to_vec(&serde_json_sign_wrapper(&request)).expect("Failed to serialize sign request"),
            MPC_SIGN_DEPOSIT, MPC_SIGN_GAS);
        let callback_args = near_sdk::serde_json::json!({"owner_id": owner_id, "record_index": index, "timestamp_ns": U64(now)});
        let callback = Promise::new(env::current_account_id()).function_call("on_eth_withdrawal_signed".to_string(),
            near_sdk::serde_json::to_vec(&callback_args).expect("Failed to serialize callback args"),
            NearToken::from_yoctonear(0), CALLBACK_GAS);
        self.store_vault(&owner_id, &vault);
        mpc.then(callback)
    }

    #[private]
    pub fn on_eth_withdrawal_signed(&mut self, owner_id: AccountId, record_index: u32, timestamp_ns: U64,
        #[callback_result] result: Result<near_sdk::serde_json::Value, PromiseError>) -> bool {
        let mut vault = self.load_vault(&owner_id);
        let idx = record_index as usize;
        if idx >= vault.withdrawals.len() { return false; }
        let is_current = vault.eth_outstanding.as_ref().is_some_and(|p|
            p.slot_index == u32::MAX && p.withdrawal_index == Some(record_index) && p.started_at_ns == timestamp_ns.0);
        if !is_current { return false; }
        match result {
            Ok(signature) => { vault.withdrawals[idx].status = WithdrawalStatus::Signed;
                vault.withdrawals[idx].tx_hash = Some(signature.to_string()); self.store_vault(&owner_id, &vault); true }
            Err(_) => { vault.withdrawals[idx].status = WithdrawalStatus::Failed;
                vault.eth_outstanding = None; self.store_vault(&owner_id, &vault); false }
        }
    }

    /// Owner sweeps one Bitcoin UTXO to the active registered P2WPKH destination.
    pub fn withdraw_btc(&mut self) -> Promise {
        let args: WithdrawBtcArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.calculate_state() != VaultState::Claimable, "Withdrawals are disabled after Claimable");
        if let Some(previous_index) = vault.btc_withdrawal_outstanding_index {
            let previous = &vault.withdrawals[previous_index as usize];
            require!(env::block_timestamp().saturating_sub(previous.timestamp_ns) >= RETRY_TIMEOUT_NS,
                "Bitcoin withdrawal retry is allowed only after 60 seconds");
            require!(previous.txid.as_deref() == Some(args.txid.as_str()) && previous.vout == Some(args.vout),
                "Retry must use the same UTXO because a previously signed transaction may still be broadcast");
            require!(args.fee_rate_sat_vb.0 > previous.fee_rate_sat_vb.unwrap_or(0),
                "Retry with same UTXO must provide a strictly higher fee rate");
        }
        require!(vault.btc_slot.status != SlotStatus::Pending && vault.btc_slot.status != SlotStatus::Signed,
            "A Bitcoin claim is already outstanding");
        let destination = vault.owner_btc_witness_program.clone().unwrap_or_else(|| env::panic_str("Bitcoin withdrawal destination is not registered"));
        self.assert_signing_reserve();
        let rate = args.fee_rate_sat_vb.0;
        require!(rate > 0 && rate <= MAX_BTC_FEE_RATE_SAT_VB, "Bitcoin fee rate is outside the allowed range");
        let fee = 110u64.checked_mul(rate).expect("Fee calculation overflow");
        let input = args.value_sats.0;
        require!(input > fee, "Input value must exceed fee");
        require!((fee as u128) <= (input as u128).checked_mul(5).expect("Fee cap overflow") / 100,
            "Estimated fee exceeds 5% of input value");
        let amount = input - fee;
        require!(amount >= 294, "Withdrawal output is below dust threshold (294 sats)");
        let txid_bytes = hex_to_bytes32(&args.txid);
        let sender = hex_to_bytes20(&vault.btc_sender_pubkey_hash);
        let output = BtcOutput { value_sats: amount, witness_program: hex_to_bytes20(&destination) };
        let sighash = compute_bip143_p2wpkh_sighash(2, txid_bytes, args.vout, sender, input,
            BTC_P2WPKH_SEQUENCE, &[output], 0);
        let now = env::block_timestamp();
        let index = vault.withdrawals.len() as u32;
        vault.withdrawals.push(WithdrawalRecord { timestamp_ns: now, chain: WithdrawalChain::Bitcoin,
            amount: amount as u128, destination: destination.clone(), status: WithdrawalStatus::Pending,
            tx_hash: None, reported_tx_hash: None, nonce: None, gas_price_wei: None,
            txid: Some(args.txid.clone()), vout: Some(args.vout), value_sats: Some(input), fee_rate_sat_vb: Some(rate) });
        vault.btc_withdrawal_outstanding_index = Some(index);
        let request = SignRequest { payload: sighash, path: vault.btc_derivation_path.clone(), key_version: 0 };
        let mpc = Promise::new(self.mpc_contract.clone()).function_call("sign".to_string(),
            near_sdk::serde_json::to_vec(&serde_json_sign_wrapper(&request)).expect("Failed to serialize sign request"),
            MPC_SIGN_DEPOSIT, MPC_SIGN_GAS);
        let callback_args = near_sdk::serde_json::json!({"owner_id": owner_id, "record_index": index, "timestamp_ns": U64(now)});
        let callback = Promise::new(env::current_account_id()).function_call("on_btc_withdrawal_signed".to_string(),
            near_sdk::serde_json::to_vec(&callback_args).expect("Failed to serialize callback args"),
            NearToken::from_yoctonear(0), CALLBACK_GAS);
        self.store_vault(&owner_id, &vault);
        mpc.then(callback)
    }

    #[private]
    pub fn on_btc_withdrawal_signed(&mut self, owner_id: AccountId, record_index: u32, timestamp_ns: U64,
        #[callback_result] result: Result<near_sdk::serde_json::Value, PromiseError>) -> bool {
        let mut vault = self.load_vault(&owner_id);
        let idx = record_index as usize;
        if idx >= vault.withdrawals.len() || vault.btc_withdrawal_outstanding_index != Some(record_index)
            || vault.withdrawals[idx].timestamp_ns != timestamp_ns.0 { return false; }
        match result {
            Ok(signature) => { vault.withdrawals[idx].status = WithdrawalStatus::Signed;
                vault.withdrawals[idx].tx_hash = Some(signature.to_string()); self.store_vault(&owner_id, &vault); true }
            Err(_) => { vault.withdrawals[idx].status = WithdrawalStatus::Failed;
                vault.btc_withdrawal_outstanding_index = None; self.store_vault(&owner_id, &vault); false }
        }
    }

    /// Record a broadcast transaction hash and release the chain lock. Hash reporting is unverified.
    pub fn mark_withdrawal_done(&mut self, owner_id: AccountId, chain: WithdrawalChain, tx_hash: String) {
        let mut vault = self.load_vault(&owner_id);
        let caller = env::predecessor_account_id();
        require!(caller == owner_id || vault.keepers.contains(&caller), "Only the owner or configured keeper can report a withdrawal");
        let index = match chain {
            WithdrawalChain::Ethereum => {
                require!(vault.eth_outstanding.as_ref().is_some_and(|p| p.slot_index == u32::MAX), "No Ethereum withdrawal is outstanding");
                vault.eth_outstanding.as_ref().and_then(|p| p.withdrawal_index).map(|i| i as usize)
                    .unwrap_or_else(|| env::panic_str("No signed Ethereum withdrawal found"))
            }
            WithdrawalChain::Bitcoin => vault.btc_withdrawal_outstanding_index.map(|i| i as usize)
                .unwrap_or_else(|| env::panic_str("No Bitcoin withdrawal is outstanding")),
        };
        require!(vault.withdrawals[index].status == WithdrawalStatus::Signed, "Withdrawal must be Signed before it can be reported Done");
        let normalized = normalize_tx_hash(&tx_hash, chain == WithdrawalChain::Ethereum);
        vault.withdrawals[index].reported_tx_hash = Some(normalized);
        vault.withdrawals[index].status = WithdrawalStatus::Done;
        if chain == WithdrawalChain::Ethereum {
            vault.last_done_eth_nonce = vault.withdrawals[index].nonce;
            vault.eth_outstanding = None;
        } else { vault.btc_withdrawal_outstanding_index = None; }
        self.store_vault(&owner_id, &vault);
    }

    /// Owner pings the contract to reset heartbeat timer
    pub fn ping(&mut self) {
        let owner_id = env::predecessor_account_id();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured yet");

        let current_state = vault.calculate_state();
        require!(
            current_state != VaultState::Claimable,
            "Cannot ping: vault has already reached Claimable state"
        );

        vault.last_heartbeat_timestamp_ns = env::block_timestamp();
        self.store_vault(&owner_id, &vault);
    }

    /// Claim distribution for heir's own slot.
    /// The contract builds the type 1 (EIP-2930) transfer itself:
    /// - Recipient is heir's stored address
    /// - Value is their share of the initial balance snapshot minus fee
    /// - Fixed chain ID (11155111)
    /// - Empty access list
    pub fn claim(&mut self) -> Promise {
        // Enforce exact arguments (no extra arguments like tx_payload allowed)
        let args: ClaimArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");

        let owner_id = args.owner_id;
        let heir_index = args.heir_index;
        let balance_wei = args.balance_wei;
        let nonce = args.nonce;
        let gas_price_wei = args.gas_price_wei;
        let mut vault = self.apply_matured_plan_change(&owner_id);

        require!(vault.is_configured, "Vault is not configured");
        let state = vault.calculate_state();
        require!(
            state == VaultState::Claimable,
            "Vault is not claimable yet: must pass grace period"
        );

        let idx = heir_index as usize;
        require!(idx < vault.heirs.len(), "Recipient index not in plan");

        let slot = &vault.heirs[idx];
        let caller = env::predecessor_account_id();

        // A recipient may claim manually; a configured keeper may trigger the same stored plan.
        require!(
            caller == slot.heir.near_account_id || vault.keepers.contains(&caller),
            "Only the designated recipient or a configured keeper can claim this slot"
        );
        match &vault.eth_outstanding {
            Some(pending) => {
                require!(pending.slot_index == heir_index, "Another Ethereum transaction is already outstanding");
                require!(pending.nonce == nonce.0, "Retry must use the outstanding Ethereum nonce");
            }
            None => {
                let expected_nonce = vault.last_done_eth_nonce.map(|n| n.saturating_add(1)).unwrap_or(0);
                require!(nonce.0 == expected_nonce, "Ethereum nonce must follow the last completed transaction");
            }
        }
        self.assert_signing_reserve();

        // Snapshot balance at first claim so every heir gets their equal share of the original total pool
        let total_pool_wei = match vault.snapshot_balance_wei {
            Some(snap) => snap,
            None => {
                vault.snapshot_balance_wei = Some(balance_wei.0);
                balance_wei.0
            }
        };

        let now_ns = env::block_timestamp();

        // Slot state checks & retry conditions
        match slot.status {
            SlotStatus::Unsent => {
                // First claim attempt allowed
            }
            SlotStatus::Pending => {
                // Allow retry only after 60s timeout with same nonce and higher gas price
                let elapsed = now_ns.saturating_sub(slot.last_claim_timestamp_ns);
                require!(
                    elapsed >= RETRY_TIMEOUT_NS,
                    "Slot is Pending: retry is allowed only after 60 seconds timeout"
                );
                require!(
                    nonce.0 == slot.last_nonce,
                    "Retry must use the same nonce as the pending attempt"
                );
                require!(
                    gas_price_wei.0 > slot.last_gas_price_wei.0,
                    "Retry must provide a higher gas price than the pending attempt"
                );
            }
            SlotStatus::Signed => {
                // If it was signed but broadcast failed or got stuck, heir can retry after 60s timeout
                let elapsed = now_ns.saturating_sub(slot.last_claim_timestamp_ns);
                require!(
                    elapsed >= RETRY_TIMEOUT_NS,
                    "Slot is Signed: retry is allowed only after 60 seconds timeout"
                );
                require!(
                    nonce.0 == slot.last_nonce,
                    "Retry must use the same nonce as the previous attempt"
                );
                require!(
                    gas_price_wei.0 > slot.last_gas_price_wei.0,
                    "Retry must provide a higher gas price than the previous attempt"
                );
            }
            SlotStatus::Done => {
                env::panic_str("Slot is already Done");
            }
        }

        // Calculate fee: gas_limit * gas_price
        let fee_wei = (vault.gas_limit as u128)
            .checked_mul(gas_price_wei.0)
            .expect("Fee calculation overflow");

        // 1. Absolute fee cap check (0.002 ETH)
        require!(
            fee_wei <= ABSOLUTE_FEE_CAP_WEI,
            "Estimated fee exceeds absolute fee cap (0.002 ETH)"
        );

        // Calculate heir gross share from total_pool_wei snapshot: (total_pool_wei * share_basis_points) / 10000
        let heir_gross_share = total_pool_wei
            .checked_mul(slot.heir.share_basis_points as u128)
            .expect("Gross share calculation overflow")
            / 10000;

        require!(
            heir_gross_share > fee_wei,
            "Heir gross share is insufficient to cover fee"
        );

        // 2. Fee cap check: at most 5% of heir's share
        let max_allowable_fee = heir_gross_share
            .checked_mul(5)
            .expect("Max allowable fee overflow")
            / 100;
        require!(
            fee_wei <= max_allowable_fee,
            "Estimated fee exceeds 5% of the heir's share"
        );

        // Value to transfer: heir's share minus fee
        let transfer_value = heir_gross_share - fee_wei;

        // Build EIP-2930 transaction bytes
        let recipient_bytes = hex_to_bytes20(&slot.heir.ethereum_address);
        let unsigned_tx_bytes = build_eip2930_unsigned_tx(
            SEPOLIA_CHAIN_ID,
            nonce.0,
            gas_price_wei.0,
            vault.gas_limit,
            recipient_bytes,
            transfer_value,
        );

        // Keccak256 hash
        let tx_hash_bytes = env::keccak256_array(&unsigned_tx_bytes);
        let tx_hash_hex = hex_string(&tx_hash_bytes);

        // Update slot state
        let slot_mut = &mut vault.heirs[idx];
        slot_mut.status = SlotStatus::Pending;
        slot_mut.signed_hash = Some(tx_hash_hex);
        slot_mut.last_claim_timestamp_ns = now_ns;
        slot_mut.last_nonce = nonce.0;
        slot_mut.last_gas_price_wei = gas_price_wei;
        vault.eth_outstanding = Some(EthOutstanding {
            slot_index: heir_index,
            nonce: nonce.0,
            started_at_ns: now_ns,
            withdrawal_index: None,
        });

        let sign_request = SignRequest {
            payload: tx_hash_bytes,
            path: vault.derivation_path.clone(),
            key_version: 0,
        };

        // Call MPC signer contract
        let mpc_promise = Promise::new(self.mpc_contract.clone()).function_call(
            "sign".to_string(),
            near_sdk::serde_json::to_vec(&serde_json_sign_wrapper(&sign_request))
                .expect("Failed to serialize sign request"),
            MPC_SIGN_DEPOSIT,
            MPC_SIGN_GAS,
        );

        // Attach callback
        let callback_promise = Promise::new(env::current_account_id()).function_call(
            "on_claim_signed".to_string(),
            near_sdk::serde_json::to_vec(&serde_json_callback_args(&owner_id, heir_index, now_ns))
                .expect("Failed to serialize callback args"),
            NearToken::from_yoctonear(0),
            CALLBACK_GAS,
        );

        self.store_vault(&owner_id, &vault);
        mpc_promise.then(callback_promise)
    }

    /// Callback invoked after MPC sign call finishes
    #[private]
    pub fn on_claim_signed(
        &mut self,
        owner_id: AccountId,
        heir_index: u32,
        claim_timestamp_ns: U64,
        #[callback_result] call_result: Result<near_sdk::serde_json::Value, PromiseError>,
    ) -> bool {
        let mut vault = self.load_vault(&owner_id);
        let idx = heir_index as usize;
        if idx >= vault.heirs.len() {
            return false;
        }
        // A delayed MPC response from an earlier attempt must not overwrite a newer retry.
        if claim_timestamp_ns.0 != vault.heirs[idx].last_claim_timestamp_ns
            || vault.heirs[idx].status != SlotStatus::Pending
        {
            return false;
        }

        match call_result {
            Ok(signature_value) => {
                // MPC signed successfully -> Slot state becomes Signed
                vault.heirs[idx].status = SlotStatus::Signed;
                vault.heirs[idx].tx_hash = Some(signature_value.to_string());
                self.store_vault(&owner_id, &vault);
                true
            }
            Err(_) => {
                // MPC call failed or rejected -> Revert to Unsent so it can be retried immediately
                vault.heirs[idx].status = SlotStatus::Unsent;
                if vault.eth_outstanding.as_ref().is_some_and(|pending| pending.slot_index == heir_index) {
                    vault.eth_outstanding = None;
                }
                self.store_vault(&owner_id, &vault);
                false
            }
        }
    }

    /// Heir marks slot as Done after confirming on-chain receipt on Sepolia
    pub fn mark_done(&mut self, owner_id: AccountId, heir_index: u32, tx_hash: String) {
        let mut vault = self.load_vault(&owner_id);
        let idx = heir_index as usize;
        require!(idx < vault.heirs.len(), "Recipient index not in plan");

        let slot = &mut vault.heirs[idx];
        let caller = env::predecessor_account_id();

        require!(caller == slot.heir.near_account_id || vault.keepers.contains(&caller),
            "Only the designated recipient or a configured keeper can mark this slot as Done");
        require!(
            slot.status == SlotStatus::Signed,
            "Slot must be in Signed state to be marked as Done"
        );

        slot.reported_tx_hash = Some(normalize_tx_hash(&tx_hash, true));
        vault.last_done_eth_nonce = Some(slot.last_nonce);
        slot.status = SlotStatus::Done;
        vault.eth_outstanding = None;
        self.store_vault(&owner_id, &vault);
    }

    /// Claim Bitcoin distribution.
    /// Only an heir can call it, only when Claimable, with deny_unknown_fields.
    /// Arguments: one UTXO (txid, vout, value_sats) and fee_rate_sat_vb. Exactly one input.
    /// The contract builds the outputs itself: for each heir, (input minus fee) times their bps,
    /// the last heir takes the remainder; fee = 141 vB times the fee rate, with a rate cap (<= 200 sat/vB)
    /// and a 5% of input cap; reject any output under 294 sats; reject if any heir lacks a Bitcoin program.
    /// Uses sequence 0xfffffffd, computes sighash, calls MPC sign with path bitcoin-1.
    pub fn claim_btc(&mut self) -> Promise {
        let args: ClaimBtcArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");

        let owner_id = args.owner_id.clone();
        let mut vault = self.apply_matured_plan_change(&owner_id);
        require!(vault.is_configured, "Vault is not configured");
        require!(vault.btc_withdrawal_outstanding_index.is_none(), "A Bitcoin withdrawal is already outstanding");
        let state = vault.calculate_state();
        require!(
            state == VaultState::Claimable,
            "Vault is not claimable yet: must pass grace period"
        );

        let caller = env::predecessor_account_id();
        require!(vault.heirs.iter().any(|h| h.heir.near_account_id == caller) || vault.keepers.contains(&caller),
            "Only a designated recipient or configured keeper can call claim_btc");
        self.assert_signing_reserve();

        let now_ns = env::block_timestamp();

        // BTC Slot state checks & retry conditions
        match vault.btc_slot.status {
            SlotStatus::Unsent => {
                // First claim attempt allowed
            }
            SlotStatus::Pending | SlotStatus::Signed => {
                // Allow retry only after 60s timeout
                let elapsed = now_ns.saturating_sub(vault.btc_slot.last_claim_timestamp_ns);
                require!(
                    elapsed >= RETRY_TIMEOUT_NS,
                    "Retry is allowed only after 60 seconds timeout"
                );
                let (previous_txid, previous_vout, previous_fee_rate) = if vault.btc_slot.status == SlotStatus::Pending {
                    (vault.btc_slot.pending_txid.as_ref(), vault.btc_slot.pending_vout,
                        vault.btc_slot.pending_fee_rate_sat_vb)
                } else {
                    (vault.btc_slot.last_txid.as_ref(), vault.btc_slot.last_vout,
                        vault.btc_slot.last_fee_rate_sat_vb)
                };
                let previous_txid = previous_txid.unwrap_or_else(||
                    env::panic_str("BTC retry is blocked because the previous UTXO was not recorded"));
                let previous_vout = previous_vout.unwrap_or_else(||
                    env::panic_str("BTC retry is blocked because the previous UTXO was not recorded"));
                let previous_fee_rate = previous_fee_rate.unwrap_or_else(||
                    env::panic_str("BTC retry is blocked because the previous fee rate was not recorded"));
                require!(args.txid == *previous_txid && args.vout == previous_vout,
                    "BTC retry must use the same UTXO because the prior signature may still be broadcast");
                require!(args.fee_rate_sat_vb.0 > previous_fee_rate,
                    "Retry with same UTXO must provide a strictly higher fee rate than the previous attempt");
            }
            SlotStatus::Done => {
                env::panic_str("BTC slot is already Done");
            }
        }

        // Fee calculation, caps, dust check, and output calculation via pure function
        let fee_rate = args.fee_rate_sat_vb.0;
        let input_value = args.value_sats.0;

        let heirs_config: Vec<HeirConfig> = vault.heirs.iter().map(|h| h.heir.clone()).collect();
        let outputs = calculate_btc_outputs(input_value, fee_rate, &heirs_config)
            .unwrap_or_else(|err| env::panic_str(&err));

        let txid_bytes = hex_to_bytes32(&args.txid);
        let sender_pubkey_hash = hex_to_bytes20(&vault.btc_sender_pubkey_hash);

        // Compute BIP-143 sighash (version 2, sequence 0xfffffffd, locktime 0)
        let sighash = compute_bip143_p2wpkh_sighash(
            2,
            txid_bytes,
            args.vout,
            sender_pubkey_hash,
            input_value,
            BTC_P2WPKH_SEQUENCE,
            &outputs,
            0,
        );

        let sighash_hex = hex_string(&sighash);

        // Track in-flight request; do not commit hash, signature, UTXO, or fee rate until callback succeeds
        let previous_status = vault.btc_slot.status;
        vault.btc_slot.status = SlotStatus::Pending;
        vault.btc_slot.last_claim_timestamp_ns = now_ns;
        vault.btc_slot.pending_txid = Some(args.txid.clone());
        vault.btc_slot.pending_vout = Some(args.vout);
        vault.btc_slot.pending_fee_rate_sat_vb = Some(fee_rate);

        let sign_request = SignRequest {
            payload: sighash,
            path: vault.btc_derivation_path.clone(),
            key_version: 0,
        };

        // Call MPC signer contract
        let mpc_promise = Promise::new(self.mpc_contract.clone()).function_call(
            "sign".to_string(),
            near_sdk::serde_json::to_vec(&serde_json_sign_wrapper(&sign_request))
                .expect("Failed to serialize sign request"),
            MPC_SIGN_DEPOSIT,
            MPC_SIGN_GAS,
        );

        // Attach callback passing sighash, UTXO, fee rate, previous status, and claim timestamp
        let callback_args = near_sdk::serde_json::json!({
            "owner_id": owner_id,
            "sighash_hex": sighash_hex,
            "txid": args.txid,
            "vout": args.vout,
            "fee_rate_sat_vb": args.fee_rate_sat_vb,
            "previous_status": previous_status,
            "claim_timestamp_ns": near_sdk::json_types::U64(now_ns),
        });

        let callback_promise = Promise::new(env::current_account_id()).function_call(
            "on_btc_claim_signed".to_string(),
            near_sdk::serde_json::to_vec(&callback_args).expect("Failed to serialize callback args"),
            NearToken::from_yoctonear(0),
            CALLBACK_GAS,
        );

        self.store_vault(&owner_id, &vault);
        mpc_promise.then(callback_promise)
    }

    #[private]
    pub fn on_btc_claim_signed(
        &mut self,
        owner_id: AccountId,
        sighash_hex: String,
        txid: String,
        vout: u32,
        fee_rate_sat_vb: U64,
        previous_status: SlotStatus,
        claim_timestamp_ns: Option<U64>,
        #[callback_result] call_result: Result<near_sdk::serde_json::Value, PromiseError>,
    ) -> bool {
        let mut vault = self.load_vault(&owner_id);
        // A failed or late callback from an older attempt does not overwrite a newer attempt
        if let Some(ts) = claim_timestamp_ns {
            if ts.0 < vault.btc_slot.last_claim_timestamp_ns {
                return false;
            }
        }
        if vault.btc_slot.status == SlotStatus::Done {
            return false;
        }

        match call_result {
            Ok(signature_value) => {
                // MPC signed successfully -> commit hash, signature, UTXO, and fee rate together
                vault.btc_slot.status = SlotStatus::Signed;
                vault.btc_slot.signed_hash = Some(sighash_hex);
                vault.btc_slot.tx_hash = Some(signature_value.to_string());
                vault.btc_slot.last_txid = Some(txid);
                vault.btc_slot.last_vout = Some(vout);
                vault.btc_slot.last_fee_rate_sat_vb = Some(fee_rate_sat_vb.0);
                vault.btc_slot.pending_txid = None;
                vault.btc_slot.pending_vout = None;
                vault.btc_slot.pending_fee_rate_sat_vb = None;
                vault.btc_slot.confirmations.clear();
                self.store_vault(&owner_id, &vault);
                true
            }
            Err(_) => {
                // A failed retry after a signed transaction keeps that signature and its UTXO.
                if previous_status == SlotStatus::Signed {
                    vault.btc_slot.status = SlotStatus::Signed;
                    vault.btc_slot.pending_txid = None;
                    vault.btc_slot.pending_vout = None;
                    vault.btc_slot.pending_fee_rate_sat_vb = None;
                } else if previous_status == SlotStatus::Pending {
                    // An earlier MPC call may still have produced a signature even if this retry failed.
                    // Keep the same-input lock so a different UTXO cannot create a second live spend.
                    vault.btc_slot.status = SlotStatus::Pending;
                } else {
                    vault.btc_slot.status = SlotStatus::Unsent;
                    vault.btc_slot.pending_txid = None;
                    vault.btc_slot.pending_vout = None;
                    vault.btc_slot.pending_fee_rate_sat_vb = None;
                }
                self.store_vault(&owner_id, &vault);
                false
            }
        }
    }

    /// A keeper can report completion; otherwise every recipient confirms the same transaction hash.
    pub fn mark_btc_done(&mut self, owner_id: AccountId, tx_hash: String) {
        let mut vault = self.load_vault(&owner_id);
        let caller = env::predecessor_account_id();
        let is_keeper = vault.keepers.contains(&caller);
        let is_recipient = vault.heirs.iter().any(|h| h.heir.near_account_id == caller);
        require!(is_keeper || is_recipient, "Only a configured keeper or recipient can mark BTC as Done");
        require!(
            vault.btc_slot.status != SlotStatus::Done,
            "BTC slot is already Done"
        );
        require!(
            vault.btc_slot.status == SlotStatus::Signed,
            "BTC slot must be in Signed state to be marked as Done"
        );

        let normalized_hash = normalize_tx_hash(&tx_hash, false);
        if let Some(existing) = &vault.btc_slot.reported_tx_hash {
            require!(existing == &normalized_hash, "BTC transaction hash does not match the existing report");
        } else {
            vault.btc_slot.reported_tx_hash = Some(normalized_hash);
        }
        if is_keeper {
            vault.btc_slot.status = SlotStatus::Done;
            self.store_vault(&owner_id, &vault);
            return;
        }

        if !vault.btc_slot.confirmations.contains(&caller) {
            vault.btc_slot.confirmations.push(caller);
        }

        let all_confirmed = vault
            .heirs
            .iter()
            .all(|h| vault.btc_slot.confirmations.contains(&h.heir.near_account_id));

        if all_confirmed {
            vault.btc_slot.status = SlotStatus::Done;
        }
        self.store_vault(&owner_id, &vault);
    }

    /// Get one owner's current state and timers. Missing owners have no vault yet.
    pub fn status(&self, owner_id: AccountId) -> Option<VaultStatusView> {
        self.vaults
            .get(&owner_id)
            .map(|vault| self.status_for(vault))
    }

    /// Number of configured owner records. Used by keeper services to paginate the registry.
    pub fn owner_count(&self) -> u32 {
        self.owner_ids.len()
    }

    /// Paginated registry of configured owners for keeper discovery.
    /// The page size is capped so callers cannot request an unbounded response.
    pub fn list_owners(&self, from_index: Option<u32>, limit: Option<u32>) -> Vec<AccountId> {
        let total = self.owner_ids.len();
        let start = from_index.unwrap_or(0).min(total);
        let page_size = limit.unwrap_or(50).min(MAX_OWNER_PAGE_SIZE);
        let end = start.saturating_add(page_size).min(total);
        (start..end)
            .filter_map(|index| self.owner_ids.get(index).cloned())
            .collect()
    }
}

impl VaultRecord {
    fn calculate_state(&self) -> VaultState {
        if !self.is_configured {
            return VaultState::Uninitialized;
        }

        let elapsed_ns = env::block_timestamp().saturating_sub(self.last_heartbeat_timestamp_ns);

        let warning_threshold_ns = self.heartbeat_interval_ns * 3 / 4;
        let grace_start_ns = self.heartbeat_interval_ns;
        let claimable_start_ns = self.heartbeat_interval_ns + self.grace_period_ns;

        if elapsed_ns >= claimable_start_ns {
            VaultState::Claimable
        } else if elapsed_ns >= grace_start_ns {
            VaultState::Grace
        } else if elapsed_ns >= warning_threshold_ns {
            VaultState::Warning
        } else {
            VaultState::Active
        }
    }
}

fn normalize_eth_address(address: &str) -> String {
    let clean = address.strip_prefix("0x").or_else(|| address.strip_prefix("0X")).unwrap_or(address);
    require!(clean.len() == 40 && clean.chars().all(|c| c.is_ascii_hexdigit()), "Invalid Ethereum address format");
    format!("0x{}", clean.to_ascii_lowercase())
}

fn normalize_witness_program(program: &str) -> String {
    let clean = program.strip_prefix("0x").or_else(|| program.strip_prefix("0X")).unwrap_or(program);
    require!(clean.len() == 40 && clean.chars().all(|c| c.is_ascii_hexdigit()), "Invalid Bitcoin 20-byte witness program");
    clean.to_ascii_lowercase()
}

fn new_heir_slot(heir: HeirConfig) -> HeirSlot {
    HeirSlot { heir, status: SlotStatus::Unsent, tx_hash: None, reported_tx_hash: None,
        signed_hash: None, last_claim_timestamp_ns: 0, last_nonce: 0, last_gas_price_wei: U128(0) }
}

fn validate_heirs(heirs: &[HeirConfig]) {
    require!(!heirs.is_empty(), "Recipients list cannot be empty");
    let total: u32 = heirs.iter().map(|h| h.share_basis_points).sum();
    require!(total == 10_000, "Total recipient shares must equal 10000 (100%)");
    for heir in heirs {
        normalize_eth_address(&heir.ethereum_address);
        if let Some(program) = &heir.bitcoin_witness_program { normalize_witness_program(program); }
    }
}

fn has_distribution_started(vault: &VaultRecord) -> bool {
    vault.heirs.iter().any(|slot| slot.status != SlotStatus::Unsent) || vault.btc_slot.status != SlotStatus::Unsent
}

pub fn hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn normalize_tx_hash(hash: &str, ethereum: bool) -> String {
    let normalized = hash.trim().to_ascii_lowercase();
    if ethereum {
        require!(normalized.starts_with("0x") && normalized.len() == 66, "Invalid Ethereum transaction hash");
        require!(normalized[2..].chars().all(|c| c.is_ascii_hexdigit()), "Invalid Ethereum transaction hash");
    } else {
        let clean = normalized.strip_prefix("0x").unwrap_or(&normalized);
        require!(clean.len() == 64 && clean.chars().all(|c| c.is_ascii_hexdigit()), "Invalid Bitcoin transaction hash");
        return clean.to_string();
    }
    normalized
}

fn serde_json_sign_wrapper(req: &SignRequest) -> near_sdk::serde_json::Value {
    near_sdk::serde_json::json!({
        "request": {
            "payload": req.payload,
            "path": req.path,
            "key_version": req.key_version
        }
    })
}

fn serde_json_callback_args(owner_id: &AccountId, heir_index: u32, claim_timestamp_ns: u64) -> near_sdk::serde_json::Value {
    near_sdk::serde_json::json!({
        "owner_id": owner_id,
        "heir_index": heir_index,
        "claim_timestamp_ns": U64(claim_timestamp_ns)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_mut(contract: &mut VaultContract) -> &mut VaultRecord {
        let owner: AccountId = "owner.testnet".parse().unwrap();
        contract.vaults.get_mut(&owner).unwrap()
    }

    fn setup_test(
        contract: &mut VaultContract,
        heirs: Vec<HeirConfig>,
        heartbeat_interval_sec: u64,
        grace_period_sec: u64,
        gas_limit: Option<U64>,
    ) {
        contract.setup(
            heirs,
            heartbeat_interval_sec,
            grace_period_sec,
            gas_limit,
            BTC_SENDER_PUBKEY_HASH.to_string(),
            None,
        );
    }

    fn setup_test_with_keepers(
        contract: &mut VaultContract,
        heirs: Vec<HeirConfig>,
        heartbeat_interval_sec: u64,
        grace_period_sec: u64,
        gas_limit: Option<U64>,
        keepers: Vec<AccountId>,
    ) {
        contract.setup_with_keepers(
            heirs,
            heartbeat_interval_sec,
            grace_period_sec,
            gas_limit,
            BTC_SENDER_PUBKEY_HASH.to_string(),
            keepers,
        );
    }

    fn one_valid_heir() -> Vec<HeirConfig> {
        vec![HeirConfig {
            near_account_id: "recipient.testnet".parse().unwrap(),
            ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
            bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
            share_basis_points: 10000,
        }]
    }

    fn set_test_destinations(contract: &mut VaultContract) {
        let vault = record_mut(contract);
        vault.owner_eth_address = Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        vault.owner_btc_witness_program = Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());
    }

    #[test]
    fn test_setup_records_explicit_keepers_and_signing_reserve() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let keeper: AccountId = "keeper.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(5));
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        assert!(contract.status(owner.clone()).is_none(), "a new owner has no vault record before setup");
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![keeper.clone()]);
        let status = contract.status(owner.clone()).unwrap();
        assert_eq!(status.keepers, vec![keeper]);
        assert_eq!(status.signing_deposit_yocto.0, MPC_SIGN_DEPOSIT.as_yoctonear());
        assert_eq!(status.near_signing_reserve_yocto.0, 5);
    }

    #[test]
    #[should_panic(expected = "Only one keeper may be configured")]
    fn test_setup_rejects_more_than_one_keeper() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        let keepers = ["keeper1.testnet", "keeper2.testnet"]
            .into_iter().map(|id| id.parse().unwrap()).collect();
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, keepers);
    }

    #[test]
    #[should_panic(expected = "Keeper must be separate from the owner")]
    fn test_setup_rejects_owner_as_keeper() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![owner]);
    }

    #[test]
    #[should_panic(expected = "Heartbeat interval cannot exceed 365 days")]
    fn test_setup_rejects_heartbeat_interval_above_one_year() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let mut context = VMContextBuilder::new();
        context.predecessor_account_id("owner.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), MAX_INTERVAL_SEC + 1, 10, None);
    }

    #[test]
    #[should_panic(expected = "Grace period cannot exceed 365 days")]
    fn test_setup_rejects_grace_period_above_one_year() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let mut context = VMContextBuilder::new();
        context.predecessor_account_id("owner.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, MAX_INTERVAL_SEC + 1, None);
    }

    #[test]
    fn test_setup_accepts_one_year_interval_and_grace_boundaries() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let mut context = VMContextBuilder::new();
        context.predecessor_account_id("owner.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), MAX_INTERVAL_SEC, MAX_INTERVAL_SEC, None);
        let status = contract.status("owner.testnet".parse().unwrap()).unwrap();
        assert_eq!(status.heartbeat_interval_sec.0, MAX_INTERVAL_SEC);
        assert_eq!(status.grace_period_sec.0, MAX_INTERVAL_SEC);
    }

    #[test]
    #[should_panic(expected = "Another Ethereum transaction is already outstanding")]
    fn test_eth_claim_rejects_second_slot_while_first_is_pending() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let recipient_a: AccountId = "recipient.testnet".parse().unwrap();
        let recipient_b: AccountId = "recipient-b.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(5));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);
        let mut second_slot = record_mut(&mut contract).heirs[0].clone();
        second_slot.heir.near_account_id = recipient_b.clone();
        second_slot.heir.share_basis_points = 5000;
        record_mut(&mut contract).heirs[0].heir.share_basis_points = 5000;
        record_mut(&mut contract).heirs.push(second_slot);

        let first_args = near_sdk::serde_json::json!({
            "owner_id": owner,
            "heir_index": 0,
            "balance_wei": U128(2_000_000_000_000_000_000u128),
            "nonce": U64(0),
            "gas_price_wei": U128(1_000_000_000)
        });
        context
            .predecessor_account_id(recipient_a)
            .block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&first_args).unwrap().into();
        testing_env!(context.build());
        let _first = contract.claim();
        assert_eq!(record_mut(&mut contract).eth_outstanding.as_ref().unwrap().nonce, 0);

        let second_args = near_sdk::serde_json::json!({
            "owner_id": "owner.testnet",
            "heir_index": 1,
            "balance_wei": U128(2_000_000_000_000_000_000u128),
            "nonce": U64(1),
            "gas_price_wei": U128(1_000_000_000)
        });
        context.predecessor_account_id(recipient_b).block_timestamp(23_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&second_args).unwrap().into();
        testing_env!(context.build());
        let _second = contract.claim();
    }

    #[test]
    #[should_panic(expected = "Ethereum nonce must follow the last completed transaction")]
    fn test_eth_claim_rejects_arbitrarily_high_initial_nonce() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let recipient: AccountId = "recipient.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(5));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);
        let args = near_sdk::serde_json::json!({
            "owner_id": owner,
            "heir_index": 0,
            "balance_wei": U128(1_000_000_000_000_000_000u128),
            "nonce": U64(10_000),
            "gas_price_wei": U128(1_000_000_000)
        });
        context
            .predecessor_account_id(recipient)
            .block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.claim();
    }

    #[test]
    fn test_two_owners_have_isolated_records_timers_and_signing_paths() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner_a: AccountId = "owner-a.testnet".parse().unwrap();
        let owner_b: AccountId = "owner-b.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner_a.clone())
            .current_account_id("ember.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);

        context
            .predecessor_account_id(owner_b.clone())
            .block_timestamp(2_000_000_000);
        testing_env!(context.build());
        setup_test(&mut contract, one_valid_heir(), 20, 10, None);

        let status_a = contract.status(owner_a.clone()).unwrap();
        let status_b = contract.status(owner_b.clone()).unwrap();
        assert_eq!(contract.owner_count(), 2);
        assert_eq!(contract.list_owners(None, Some(1)), vec![owner_a.clone()]);
        assert_eq!(contract.list_owners(Some(1), Some(1)), vec![owner_b.clone()]);
        assert!(contract.list_owners(Some(2), Some(10)).is_empty());
        assert_ne!(status_a.derivation_path, status_b.derivation_path);
        assert_ne!(status_a.btc_derivation_path, status_b.btc_derivation_path);
        assert_eq!(status_a.last_heartbeat_timestamp_ns.0, 1_000_000_000);
        assert_eq!(status_b.last_heartbeat_timestamp_ns.0, 2_000_000_000);

        context
            .predecessor_account_id(owner_a.clone())
            .block_timestamp(3_000_000_000);
        testing_env!(context.build());
        contract.ping();

        assert_eq!(
            contract.status(owner_a).unwrap().last_heartbeat_timestamp_ns.0,
            3_000_000_000
        );
        assert_eq!(
            contract.status(owner_b).unwrap().last_heartbeat_timestamp_ns.0,
            2_000_000_000
        );
    }

    #[test]
    #[should_panic(expected = "Only the designated recipient or a configured keeper can claim this slot")]
    fn test_keeper_for_one_owner_cannot_claim_another_owners_vault() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner_a: AccountId = "owner-a.testnet".parse().unwrap();
        let owner_b: AccountId = "owner-b.testnet".parse().unwrap();
        let keeper_a: AccountId = "keeper-a.testnet".parse().unwrap();
        let keeper_b: AccountId = "keeper-b.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner_a.clone())
            .current_account_id("ember.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(
            &mut contract,
            one_valid_heir(),
            10,
            10,
            None,
            vec![keeper_a],
        );

        context
            .predecessor_account_id(owner_b.clone())
            .block_timestamp(2_000_000_000);
        testing_env!(context.build());
        setup_test_with_keepers(
            &mut contract,
            one_valid_heir(),
            10,
            10,
            None,
            vec![keeper_b.clone()],
        );

        let args = near_sdk::serde_json::json!({
            "owner_id": owner_a,
            "heir_index": 0,
            "balance_wei": U128(1_000_000_000_000_000_000),
            "nonce": U64(0),
            "gas_price_wei": U128(1_000_000_000)
        });
        context
            .predecessor_account_id(keeper_b)
            .block_timestamp(25_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.claim();
    }

    #[test]
    fn test_eth_mpc_callback_updates_only_named_owner_record() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner_a: AccountId = "owner-a.testnet".parse().unwrap();
        let owner_b: AccountId = "owner-b.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner_a.clone())
            .current_account_id("ember.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);

        context.predecessor_account_id(owner_b.clone());
        testing_env!(context.build());
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);

        contract.vaults.get_mut(&owner_a).unwrap().heirs[0].status = SlotStatus::Pending;
        contract.on_claim_signed(
            owner_a.clone(),
            0,
            U64(0),
            Ok(near_sdk::serde_json::json!("signed-a")),
        );
        assert_eq!(
            contract.status(owner_a).unwrap().heirs[0].tx_hash.as_deref(),
            Some("\"signed-a\"")
        );
        assert_eq!(contract.status(owner_b).unwrap().heirs[0].tx_hash, None);
    }

    #[test]
    fn test_stale_eth_mpc_callback_cannot_overwrite_newer_retry() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner-stale.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("ember.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);
        let record = contract.vaults.get_mut(&owner).unwrap();
        record.heirs[0].status = SlotStatus::Pending;
        record.heirs[0].last_claim_timestamp_ns = 200;
        record.heirs[0].last_nonce = 4;

        let applied = contract.on_claim_signed(
            owner.clone(),
            0,
            U64(100),
            Ok(near_sdk::serde_json::json!({"signature": "old"})),
        );

        assert!(!applied);
        let status = contract.status(owner).unwrap();
        assert_eq!(status.heirs[0].status, SlotStatus::Pending);
        assert_eq!(status.heirs[0].tx_hash, None);
        assert_eq!(status.heirs[0].last_claim_timestamp_ns.0, 200);
    }

    #[test]
    fn test_btc_mpc_callback_updates_only_named_owner_record() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner_a: AccountId = "owner-a.testnet".parse().unwrap();
        let owner_b: AccountId = "owner-b.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner_a.clone())
            .current_account_id("ember.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);

        context.predecessor_account_id(owner_b.clone());
        testing_env!(context.build());
        setup_test(&mut contract, one_valid_heir(), 10, 10, None);

        contract.on_btc_claim_signed(
            owner_a.clone(),
            "sighash-a".to_string(),
            "txid-a".to_string(),
            0,
            U64(2),
            SlotStatus::Unsent,
            Some(U64(1_000_000_000)),
            Ok(near_sdk::serde_json::json!("signed-a")),
        );
        let status_a = contract.status(owner_a).unwrap();
        let status_b = contract.status(owner_b).unwrap();
        assert_eq!(status_a.btc_slot.status, SlotStatus::Signed);
        assert_eq!(status_a.btc_slot.tx_hash.as_deref(), Some("\"signed-a\""));
        assert_eq!(status_b.btc_slot.status, SlotStatus::Unsent);
        assert_eq!(status_b.btc_slot.tx_hash, None);
    }

    #[test]
    fn test_keeper_can_trigger_eth_claim_and_report_hash() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let keeper: AccountId = "keeper.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(5));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![keeper.clone()]);

        let args = near_sdk::serde_json::json!({
            "owner_id": owner,
            "heir_index": 0,
            "balance_wei": U128(1_000_000_000_000_000_000),
            "nonce": U64(0),
            "gas_price_wei": U128(1_000_000_000)
        });
        let mut context = context;
        context
            .predecessor_account_id(keeper.clone())
            .block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.claim();
        assert_eq!(record_mut(&mut contract).heirs[0].status, SlotStatus::Pending);

        record_mut(&mut contract).heirs[0].status = SlotStatus::Signed;
        let reported_hash = format!("0x{}", "ab".repeat(32));
        contract.mark_done(owner.clone(), 0, reported_hash.clone());
        assert_eq!(record_mut(&mut contract).heirs[0].status, SlotStatus::Done);
        assert_eq!(contract.status(owner.clone()).unwrap().heirs[0].reported_tx_hash.as_deref(), Some(reported_hash.as_str()));
    }

    #[test]
    #[should_panic(expected = "Contract NEAR balance is too low to pay MPC signing deposit")]
    fn test_eth_claim_rejects_low_contract_signing_reserve() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let keeper: AccountId = "keeper.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(0));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![keeper.clone()]);
        let args = near_sdk::serde_json::json!({
            "owner_id": owner,
            "heir_index": 0,
            "balance_wei": U128(1_000_000_000_000_000_000),
            "nonce": U64(0),
            "gas_price_wei": U128(1_000_000_000)
        });
        context
            .predecessor_account_id(keeper)
            .block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.claim();
    }

    #[test]
    fn test_keeper_can_mark_btc_done_and_status_records_txid() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let keeper: AccountId = "keeper.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![keeper.clone()]);
        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        context.predecessor_account_id(keeper);
        testing_env!(context.build());

        let txid = "cd".repeat(32);
        contract.mark_btc_done(owner.clone(), txid.clone());
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Done);
        assert_eq!(contract.status(owner.clone()).unwrap().btc_slot.reported_tx_hash.as_deref(), Some(txid.as_str()));
    }

    #[test]
    fn test_keeper_can_trigger_btc_claim() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let keeper: AccountId = "keeper.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(5));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test_with_keepers(&mut contract, one_valid_heir(), 10, 10, None, vec![keeper.clone()]);

        let args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122_462),
            fee_rate_sat_vb: U64(2),
        };
        let mut context = context;
        context.predecessor_account_id(keeper).block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.claim_btc();
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Pending);
    }

    #[test]
    fn test_eip2930_unsigned_serialization_and_hash() {
        let chain_id = 11155111u64;
        let nonce = 2u64;
        let gas_price = 200_000_000u128;
        let gas_limit = 250_000u64;
        let to = hex_to_bytes20("0x3535353535353535353535353535353535353535");
        let value = 20_000_000_000_000_000u128;

        let signing_bytes = build_eip2930_unsigned_tx(
            chain_id,
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
        );

        let signing_bytes_hex = hex_string(&signing_bytes);
        assert_eq!(
            signing_bytes_hex,
            "01ed83aa36a702840bebc2008303d09094353535353535353535353535353535353535353587470de4df82000080c0",
            "Signing bytes mismatch"
        );

        let hash_bytes = env::keccak256_array(&signing_bytes);
        let hash_hex = hex_string(&hash_bytes);
        assert_eq!(
            hash_hex,
            "e88cf8dc4168bc8347df111e1b49e7f37a9cc9fc7fa32c61d17ce497e3ae4941",
            "Keccak256 hash mismatch"
        );
    }

    #[test]
    fn test_bip143_p2wpkh_sighash_vector1() {
        let txid = hex_to_bytes32("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72");
        let vout = 1u32;
        let value = 122462u64;
        let sequence = 0xffffffffu32;
        let version = 2u32;
        let locktime = 0u32;
        let program = hex_to_bytes20("8914c1af8c64acb5e0080fd4de181d30e4456ad4");
        let sender_pubkey_hash = hex_to_bytes20("77ed9969481823854b8c850b4596a16030bb3480");

        let outputs = vec![BtcOutput {
            value_sats: 122182,
            witness_program: program,
        }];

        let sighash = compute_bip143_p2wpkh_sighash(
            version,
            txid,
            vout,
            sender_pubkey_hash,
            value,
            sequence,
            &outputs,
            locktime,
        );

        let sighash_hex = hex_string(&sighash);
        assert_eq!(
            sighash_hex,
            "873cb185a71fb72dd44ccef5d857f9e46c21d56ff0cc1659ef250ee9cf56c974",
            "BIP-143 sighash vector 1 mismatch"
        );
    }

    #[test]
    fn test_bip143_p2wpkh_sighash_vector2() {
        let txid = hex_to_bytes32("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72");
        let vout = 1u32;
        let value = 122462u64;
        let sequence = 0xfffffffd_u32;
        let version = 2u32;
        let locktime = 0u32;
        let program1 = hex_to_bytes20("8914c1af8c64acb5e0080fd4de181d30e4456ad4");
        let program2 = hex_to_bytes20("77ed9969481823854b8c850b4596a16030bb3480");
        let sender_pubkey_hash = hex_to_bytes20("77ed9969481823854b8c850b4596a16030bb3480");

        let outputs = vec![
            BtcOutput {
                value_sats: 61090,
                witness_program: program1,
            },
            BtcOutput {
                value_sats: 61090,
                witness_program: program2,
            },
        ];

        let sighash = compute_bip143_p2wpkh_sighash(
            version,
            txid,
            vout,
            sender_pubkey_hash,
            value,
            sequence,
            &outputs,
            locktime,
        );

        let sighash_hex = hex_string(&sighash);
        assert_eq!(
            sighash_hex,
            "111d2118913b13da5df4cacbd0df9628ae80f72a08fd823f027e53cbb224ba3a",
            "BIP-143 sighash vector 2 mismatch"
        );
    }

    #[test]
    fn test_claim_btc_args_deserialization() {
        let json_data = r#"{
            "owner_id": "owner.testnet",
            "txid": "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72",
            "vout": 1,
            "value_sats": "122462",
            "fee_rate_sat_vb": 2
        }"#;

        let parsed: ClaimBtcArgs = near_sdk::serde_json::from_str(json_data).expect("Should parse");
        assert_eq!(parsed.vout, 1);
        assert_eq!(parsed.value_sats.0, 122462);
        assert_eq!(parsed.fee_rate_sat_vb.0, 2);

        // Unknown field must be rejected
        let invalid_json = r#"{
            "owner_id": "owner.testnet",
            "txid": "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72",
            "vout": 1,
            "value_sats": "122462",
            "fee_rate_sat_vb": 2,
            "extra_field": 123
        }"#;
        assert!(near_sdk::serde_json::from_str::<ClaimBtcArgs>(invalid_json).is_err());
    }

    #[test]
    fn test_calculate_btc_outputs_remainder_to_last_heir() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 3333,
            },
            HeirConfig {
                near_account_id: "heir2.testnet".parse().unwrap(),
                ethereum_address: "0x2222222222222222222222222222222222222222".to_string(),
                bitcoin_witness_program: Some("77ed9969481823854b8c850b4596a16030bb3480".to_string()),
                share_basis_points: 3333,
            },
            HeirConfig {
                near_account_id: "heir3.testnet".parse().unwrap(),
                ethereum_address: "0x3333333333333333333333333333333333333333".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 3334,
            },
        ];

        let input_value_sats = 100_000u64;
        let fee_rate_sat_vb = 1u64; // fee = 172 sats (11 + 68 + 31*3), pool = 99828 sats
        let outputs = calculate_btc_outputs(input_value_sats, fee_rate_sat_vb, &heirs)
            .expect("Calculation should succeed");

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].value_sats, 33272);
        assert_eq!(outputs[1].value_sats, 33272);
        // Remainder given to last heir: 99828 - 33272 - 33272 = 33284
        assert_eq!(outputs[2].value_sats, 33284);
        let total_allocated: u64 = outputs.iter().map(|o| o.value_sats).sum();
        assert_eq!(total_allocated, 99828);
    }

    #[test]
    fn test_calculate_btc_outputs_dust_rejection() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10,
            },
            HeirConfig {
                near_account_id: "heir2.testnet".parse().unwrap(),
                ethereum_address: "0x2222222222222222222222222222222222222222".to_string(),
                bitcoin_witness_program: Some("77ed9969481823854b8c850b4596a16030bb3480".to_string()),
                share_basis_points: 9990,
            },
        ];

        // 100,000 sats input, fee 141 -> pool 99,859 sats.
        // Heir 1 gets 10 bps (99 sats) < 294 dust threshold.
        let res = calculate_btc_outputs(100_000, 1, &heirs);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("dust threshold"));
    }

    #[test]
    fn test_calculate_btc_outputs_fee_over_rate_cap() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            },
        ];

        let res = calculate_btc_outputs(1_000_000, 201, &heirs);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("exceeds maximum cap"));
    }

    #[test]
    fn test_calculate_btc_outputs_fee_over_5_percent() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            },
        ];

        // Input 10,000 sats. 5% cap is 500 sats. For 1 heir, vbytes = 110.
        // At fee rate 5 sat/vB, fee = 110 * 5 = 550 sats (> 500 sats cap)
        let res = calculate_btc_outputs(10_000, 5, &heirs);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("exceeds 5% of input value"));
    }

    #[test]
    fn test_calculate_btc_outputs_heir_with_no_bitcoin_address() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: None,
                share_basis_points: 10000,
            },
        ];

        let res = calculate_btc_outputs(100_000, 1, &heirs);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Bitcoin witness program"));
    }

    #[test]
    #[should_panic(expected = "Only a designated recipient or configured keeper can call claim_btc")]
    fn test_claim_btc_non_heir_caller() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();
        let stranger: AccountId = "stranger.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1,
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10, // 10s heartbeat
            10, // 10s grace
            None,
        );

        // Advance time past heartbeat + grace (25s later -> Claimable)
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(2),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(stranger)
            .block_timestamp(26_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
    }

    #[test]
    #[should_panic(expected = "Vault is not claimable yet: must pass grace period")]
    fn test_claim_btc_early_claim() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            100, // 100s heartbeat
            60,  // 60s grace
            None,
        );

        // Call early while Active (e.g. at 5s)
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(2),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(5_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
    }

    #[test]
    #[should_panic(expected = "Only a configured keeper or recipient can mark BTC as Done")]
    fn test_mark_btc_done_non_heir_caller() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();
        let stranger: AccountId = "stranger.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1,
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;

        context.predecessor_account_id(stranger);
        testing_env!(context.build());

        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    }

    #[test]
    fn test_mark_btc_done_requires_all_heirs_and_locks() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();
        let heir2: AccountId = "heir2.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![
                HeirConfig {
                    near_account_id: heir1.clone(),
                    ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                    bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                    share_basis_points: 5000,
                },
                HeirConfig {
                    near_account_id: heir2.clone(),
                    ethereum_address: "0x2222222222222222222222222222222222222222".to_string(),
                    bitcoin_witness_program: Some("77ed9969481823854b8c850b4596a16030bb3480".to_string()),
                    share_basis_points: 5000,
                },
            ],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;

        // Heir 1 confirms: should remain Signed
        context.predecessor_account_id(heir1.clone());
        testing_env!(context.build());
        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Signed);
        assert_eq!(record_mut(&mut contract).btc_slot.confirmations.len(), 1);

        // Heir 2 confirms: all confirmed -> becomes Done
        context.predecessor_account_id(heir2);
        testing_env!(context.build());
        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Done);
        assert_eq!(record_mut(&mut contract).btc_slot.confirmations.len(), 2);
    }

    #[test]
    #[should_panic(expected = "BTC slot is already Done")]
    fn test_mark_btc_done_lock_panics_when_done() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Done;

        context.predecessor_account_id(heir1);
        testing_env!(context.build());

        // Calling mark_btc_done when already Done triggers Done lock
        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());
    }

    #[test]
    #[should_panic(expected = "Retry with same UTXO must provide a strictly higher fee rate than the previous attempt")]
    fn test_claim_btc_retry_same_or_lower_fee_rate_rejected() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        record_mut(&mut contract).btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        record_mut(&mut contract).btc_slot.last_vout = Some(1);
        record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb = Some(10);
        record_mut(&mut contract).btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance 61 seconds (eligible for retry timeout)
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(10), // same fee rate, must be rejected
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(62_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
    }

    #[test]
    #[should_panic(expected = "BTC retry must use the same UTXO because the prior signature may still be broadcast")]
    fn test_claim_btc_retry_different_utxo_rejected() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        record_mut(&mut contract).btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        record_mut(&mut contract).btc_slot.last_vout = Some(1);
        record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb = Some(10);
        record_mut(&mut contract).btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // A different coin is rejected even after the timeout because the old signed tx may still be broadcast.
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(5),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(62_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
    }

    #[test]
    fn test_claim_btc_retry_same_utxo_higher_fee_rate_succeeds() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        record_mut(&mut contract).btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        record_mut(&mut contract).btc_slot.last_vout = Some(1);
        record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb = Some(10);
        record_mut(&mut contract).btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance 61 seconds (past 60s timeout); retry with same UTXO and strictly higher fee rate (15 > 10) succeeds
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(15),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(62_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Pending);
        assert_eq!(record_mut(&mut contract).btc_slot.last_claim_timestamp_ns, 62_000_000_000);
    }

    #[test]
    #[should_panic(expected = "Retry is allowed only after 60 seconds timeout")]
    fn test_claim_btc_retry_before_timeout_rejected() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        record_mut(&mut contract).btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        record_mut(&mut contract).btc_slot.last_vout = Some(1);
        record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb = Some(10);
        record_mut(&mut contract).btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance only 30 seconds (< 60s timeout); retry is rejected
        let claim_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(15),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(31_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        let _ = contract.claim_btc();
    }

    #[test]
    fn test_mark_btc_done_one_heir_confirming_twice_counts_once() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();
        let heir2: AccountId = "heir2.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![
                HeirConfig {
                    near_account_id: heir1.clone(),
                    ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                    bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                    share_basis_points: 5000,
                },
                HeirConfig {
                    near_account_id: heir2.clone(),
                    ethereum_address: "0x2222222222222222222222222222222222222222".to_string(),
                    bitcoin_witness_program: Some("77ed9969481823854b8c850b4596a16030bb3480".to_string()),
                    share_basis_points: 5000,
                },
            ],
            10,
            10,
            None,
        );

        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;

        // Heir 1 confirms first time
        context.predecessor_account_id(heir1.clone());
        testing_env!(context.build());
        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());

        assert_eq!(record_mut(&mut contract).btc_slot.confirmations.len(), 1);
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Signed);

        // Heir 1 confirms second time - counts once, does not duplicate, status remains Signed
        contract.mark_btc_done(owner.clone(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string());

        assert_eq!(record_mut(&mut contract).btc_slot.confirmations.len(), 1);
        assert_eq!(record_mut(&mut contract).btc_slot.confirmations[0], heir1);
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Signed);
    }

    #[test]
    fn test_claim_btc_retry_while_pending_requires_same_utxo_and_higher_fee() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        let first_args = ClaimBtcArgs {
            owner_id: owner.clone(),
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(5),
        };
        context.predecessor_account_id(heir1.clone()).block_timestamp(22_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&first_args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.claim_btc();
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Pending);
        assert_eq!(record_mut(&mut contract).btc_slot.pending_txid.as_deref(), Some(first_args.txid.as_str()));

        let retry_args = ClaimBtcArgs { fee_rate_sat_vb: U64(10), ..first_args };
        context.block_timestamp(83_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&retry_args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.claim_btc();
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Pending);
        assert_eq!(record_mut(&mut contract).btc_slot.pending_fee_rate_sat_vb, Some(10));
        assert_eq!(record_mut(&mut contract).btc_slot.last_claim_timestamp_ns, 83_000_000_000);
    }

    #[test]
    fn test_on_btc_claim_signed_failed_or_late_callback_does_not_overwrite_newer_attempt() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        use near_sdk::PromiseError;

        let owner: AccountId = "owner.testnet".parse().unwrap();
        let heir1: AccountId = "heir1.testnet".parse().unwrap();
        let vault: AccountId = "vault.testnet".parse().unwrap();

        let mut context = VMContextBuilder::new();
        context
            .predecessor_account_id(owner.clone())
            .current_account_id(vault.clone())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());

        let mut contract = VaultContract::new(None);
        setup_test(&mut contract,
            vec![HeirConfig {
                near_account_id: heir1.clone(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 10000,
            }],
            10,
            10,
            None,
        );

        // on_btc_claim_signed is #[private], so caller must be vault
        context.predecessor_account_id(vault.clone());
        testing_env!(context.build());

        // Attempt 2 was initiated at t = 70_000_000_000
        record_mut(&mut contract).btc_slot.status = SlotStatus::Pending;
        record_mut(&mut contract).btc_slot.last_claim_timestamp_ns = 70_000_000_000;

        // Late failed callback arrives from Attempt 1 (timestamp t = 1_000_000_000)
        let res_err = contract.on_btc_claim_signed(owner.clone(),
            "old_hash".to_string(),
            "old_txid".to_string(),
            0,
            U64(5),
            SlotStatus::Unsent,
            Some(U64(1_000_000_000)),
            Err(PromiseError::Failed),
        );

        assert!(!res_err);
        // Status remains Pending, does not reset to Unsent
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Pending);

        // Attempt 2 succeeds and commits Signed state
        record_mut(&mut contract).btc_slot.status = SlotStatus::Signed;
        record_mut(&mut contract).btc_slot.signed_hash = Some("new_hash".to_string());
        record_mut(&mut contract).btc_slot.tx_hash = Some("new_sig".to_string());
        record_mut(&mut contract).btc_slot.last_txid = Some("new_txid".to_string());
        record_mut(&mut contract).btc_slot.last_vout = Some(1);
        record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb = Some(20);

        // Another late callback from Attempt 1 arrives with Ok
        let res_late_ok = contract.on_btc_claim_signed(owner.clone(),
            "old_hash".to_string(),
            "old_txid".to_string(),
            0,
            U64(5),
            SlotStatus::Unsent,
            Some(U64(1_000_000_000)),
            Ok(near_sdk::serde_json::json!("old_sig")),
        );

        assert!(!res_late_ok);
        // Attempt 2's signed values remain completely intact
        assert_eq!(record_mut(&mut contract).btc_slot.status, SlotStatus::Signed);
        assert_eq!(record_mut(&mut contract).btc_slot.signed_hash.as_deref(), Some("new_hash"));
        assert_eq!(record_mut(&mut contract).btc_slot.tx_hash.as_deref(), Some("new_sig"));
        assert_eq!(record_mut(&mut contract).btc_slot.last_txid.as_deref(), Some("new_txid"));
        assert_eq!(record_mut(&mut contract).btc_slot.last_vout, Some(1));
        assert_eq!(record_mut(&mut contract).btc_slot.last_fee_rate_sat_vb, Some(20));
    }

    #[test]
    fn test_calculate_btc_outputs_three_heirs_fee_size() {
        let heirs = vec![
            HeirConfig {
                near_account_id: "heir1.testnet".parse().unwrap(),
                ethereum_address: "0x1111111111111111111111111111111111111111".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 3333,
            },
            HeirConfig {
                near_account_id: "heir2.testnet".parse().unwrap(),
                ethereum_address: "0x2222222222222222222222222222222222222222".to_string(),
                bitcoin_witness_program: Some("77ed9969481823854b8c850b4596a16030bb3480".to_string()),
                share_basis_points: 3333,
            },
            HeirConfig {
                near_account_id: "heir3.testnet".parse().unwrap(),
                ethereum_address: "0x3333333333333333333333333333333333333333".to_string(),
                bitcoin_witness_program: Some("8914c1af8c64acb5e0080fd4de181d30e4456ad4".to_string()),
                share_basis_points: 3334,
            },
        ];

        // 3 heirs: vbytes = 11 + 68 + 31 * 3 = 172 vB
        assert_eq!(calculate_btc_vbytes(3), 172);

        let input_value_sats = 100_000u64;
        let fee_rate_sat_vb = 2u64; // fee = 172 * 2 = 344 sats, pool = 99,656 sats
        let outputs = calculate_btc_outputs(input_value_sats, fee_rate_sat_vb, &heirs)
            .expect("Calculation should succeed");

        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[0].value_sats, 33215);
        assert_eq!(outputs[1].value_sats, 33215);
        assert_eq!(outputs[2].value_sats, 33226);
        let total_allocated: u64 = outputs.iter().map(|o| o.value_sats).sum();
        assert_eq!(total_allocated, 99656);
        assert_eq!(input_value_sats - total_allocated, 344);
    }

    #[test]
    fn test_destination_change_is_delayed_and_owner_can_cancel() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 365 * 24 * 60 * 60, 30, None);
        set_test_destinations(&mut contract);
        context.block_timestamp(2_000_000_000);
        testing_env!(context.build());
        contract.set_withdrawal_destinations("0xcccccccccccccccccccccccccccccccccccccccc".into(),
            "dddddddddddddddddddddddddddddddddddddddd".into());
        let status = contract.status(owner.clone()).unwrap();
        assert_eq!(status.owner_eth_address.as_deref(), Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert_eq!(status.pending_plan_change.as_ref().unwrap().effective_at_ns.0,
            2_000_000_000 + PLAN_CHANGE_DELAY_NS);
        contract.cancel_plan_change();
        assert!(contract.status(owner).unwrap().pending_plan_change.is_none());
    }

    #[test]
    fn test_first_destination_registration_is_also_delayed() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), MAX_INTERVAL_SEC, MAX_INTERVAL_SEC, None);
        contract.set_withdrawal_destinations("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
        let status = contract.status(owner).unwrap();
        assert!(status.owner_eth_address.is_none());
        assert!(status.owner_btc_witness_program.is_none());
        assert!(status.pending_plan_change.is_some());
    }

    #[test]
    fn test_plan_change_stays_pending_until_fourteen_days_and_applies_old_vs_new_plan() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        let active = one_valid_heir();
        setup_test(&mut contract, active.clone(), MAX_INTERVAL_SEC, MAX_INTERVAL_SEC, None);
        let mut proposed = active[0].clone();
        proposed.ethereum_address = "0x2222222222222222222222222222222222222222".to_string();
        contract.propose_plan_change(vec![proposed.clone()], vec![]);
        assert_eq!(contract.status(owner.clone()).unwrap().heirs[0].heir.ethereum_address,
            active[0].ethereum_address);
        context.block_timestamp(1_000_000_000 + PLAN_CHANGE_DELAY_NS - 1);
        testing_env!(context.build());
        contract.apply_plan_change(owner.clone());
        assert_eq!(contract.status(owner.clone()).unwrap().heirs[0].heir.ethereum_address,
            active[0].ethereum_address);
        assert!(contract.status(owner.clone()).unwrap().pending_plan_change.is_some());
        context.block_timestamp(1_000_000_000 + PLAN_CHANGE_DELAY_NS);
        testing_env!(context.build());
        contract.apply_plan_change(owner.clone());
        assert_eq!(contract.status(owner.clone()).unwrap().heirs[0].heir.ethereum_address,
            proposed.ethereum_address);
        assert!(contract.status(owner).unwrap().pending_plan_change.is_none());
    }

    #[test]
    fn test_plan_change_is_discarded_if_claimable_begins_first() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        let active = one_valid_heir();
        setup_test(&mut contract, active.clone(), 1, 1, None);
        let mut proposed = active[0].clone();
        proposed.ethereum_address = "0x2222222222222222222222222222222222222222".to_string();
        contract.propose_plan_change(vec![proposed], vec![]);
        context.block_timestamp(3_000_000_000);
        testing_env!(context.build());
        contract.apply_plan_change(owner.clone());
        let status = contract.status(owner).unwrap();
        assert_eq!(status.state, VaultState::Claimable);
        assert!(status.pending_plan_change.is_none());
        assert_eq!(status.heirs[0].heir.ethereum_address, active[0].ethereum_address);
    }

    #[test]
    fn test_mature_destination_change_applies_after_fourteen_days() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), MAX_INTERVAL_SEC, MAX_INTERVAL_SEC, None);
        set_test_destinations(&mut contract);
        contract.set_withdrawal_destinations("0xcccccccccccccccccccccccccccccccccccccccc".into(),
            "dddddddddddddddddddddddddddddddddddddddd".into());
        context.block_timestamp(1_000_000_000 + PLAN_CHANGE_DELAY_NS);
        testing_env!(context.build());
        contract.apply_plan_change(owner.clone());
        let status = contract.status(owner).unwrap();
        assert_eq!(status.owner_eth_address.as_deref(), Some("0xcccccccccccccccccccccccccccccccccccccccc"));
        assert!(status.pending_plan_change.is_none());
    }

    #[test]
    fn test_schedule_update_restarts_timer_and_enforces_cap() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(5_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 100, 20, None);
        context.block_timestamp(50_000_000_000);
        testing_env!(context.build());
        contract.update_schedule(5, 10);
        let status = contract.status(owner).unwrap();
        assert_eq!(status.heartbeat_interval_sec.0, 5);
        assert_eq!(status.grace_period_sec.0, 10);
        assert_eq!(status.last_heartbeat_timestamp_ns.0, 50_000_000_000);
    }

    #[test]
    fn test_eth_withdrawal_uses_registered_destination_and_does_not_reset_timer() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1000, 1000, None);
        set_test_destinations(&mut contract);
        let heartbeat = contract.status(owner.clone()).unwrap().last_heartbeat_timestamp_ns.0;
        let args = near_sdk::serde_json::json!({"balance_wei": U128(1_000_000_000_000_000_000u128),
            "nonce": U64(0), "gas_price_wei": U128(1_000_000_000u128)});
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _promise = contract.withdraw_eth();
        let status = contract.status(owner.clone()).unwrap();
        assert_eq!(status.withdrawal_records.len(), 1);
        assert_eq!(status.withdrawal_records[0].destination, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(status.withdrawal_records[0].status, WithdrawalStatus::Pending);
        assert_eq!(status.eth_outstanding_nonce.unwrap().0, 0);
        assert_eq!(status.last_heartbeat_timestamp_ns.0, heartbeat);
    }

    #[test]
    fn test_eth_withdrawal_done_records_hash_and_advances_shared_nonce() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let vault_id: AccountId = "vault.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id(vault_id.clone())
            .block_timestamp(1_000_000_000).account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1000, 1000, None);
        set_test_destinations(&mut contract);
        context.context.input = br#"{"balance_wei":"1000000000000000000","nonce":"0","gas_price_wei":"1000000000"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_eth();
        context.predecessor_account_id(vault_id).block_timestamp(2_000_000_000);
        testing_env!(context.build());
        assert!(contract.on_eth_withdrawal_signed(owner.clone(), 0, U64(1_000_000_000),
            Ok(near_sdk::serde_json::json!("mpc-signature"))));
        context.predecessor_account_id(owner.clone());
        testing_env!(context.build());
        contract.mark_withdrawal_done(owner.clone(), WithdrawalChain::Ethereum,
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
        let status = contract.status(owner).unwrap();
        assert_eq!(status.withdrawal_records[0].status, WithdrawalStatus::Done);
        assert_eq!(status.withdrawal_records[0].reported_tx_hash.as_deref(),
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert_eq!(record_mut(&mut contract).last_done_eth_nonce, Some(0));
        assert!(status.eth_outstanding_nonce.is_none());
    }

    #[test]
    #[should_panic(expected = "Another Ethereum transaction is already outstanding")]
    fn test_eth_withdrawal_and_claim_share_the_nonce_lock() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let recipient: AccountId = "recipient.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000).account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1, 1, None);
        set_test_destinations(&mut contract);
        context.context.input = br#"{"balance_wei":"1000000000000000000","nonce":"0","gas_price_wei":"1000000000"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_eth();
        let claim_args = near_sdk::serde_json::json!({"owner_id":owner,"heir_index":0,
            "balance_wei":U128(1_000_000_000_000_000_000u128),"nonce":U64(0),"gas_price_wei":U128(1_000_000_000u128)});
        context.predecessor_account_id(recipient).block_timestamp(3_000_000_000);
        context.context.input = near_sdk::serde_json::to_vec(&claim_args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.claim();
    }

    #[test]
    #[should_panic(expected = "Failed to deserialize input arguments")]
    fn test_eth_withdrawal_rejects_destination_argument() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner).current_account_id("vault.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 100, 100, None);
        context.context.input = br#"{"balance_wei":"1000000000000000000","nonce":"0","gas_price_wei":"1000000000","destination":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_eth();
    }

    #[test]
    #[should_panic(expected = "Withdrawals are disabled after Claimable")]
    fn test_eth_withdrawal_rejected_in_claimable() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(1_000_000_000);
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1, 1, None);
        set_test_destinations(&mut contract);
        context.block_timestamp(3_000_000_000);
        context.context.input = br#"{"balance_wei":"1000000000000000000","nonce":"0","gas_price_wei":"1000000000"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_eth();
    }

    #[test]
    fn test_btc_withdrawal_sweeps_to_registered_address_and_locks_btc_claim() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let recipient: AccountId = "recipient.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap()).block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1, 1, None);
        set_test_destinations(&mut contract);
        let args = near_sdk::serde_json::json!({"txid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "vout":0,"value_sats":10000,"fee_rate_sat_vb":1});
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
        let record = &contract.status(owner.clone()).unwrap().withdrawal_records[0];
        assert_eq!(record.destination, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        assert_eq!(record.amount.0, 9890);
        assert_eq!(record.status, WithdrawalStatus::Pending);

        context.predecessor_account_id(recipient).block_timestamp(3_000_000_000);
        let claim_args = near_sdk::serde_json::json!({"owner_id":owner,"txid":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "vout":0,"value_sats":U64(10000),"fee_rate_sat_vb":U64(1)});
        context.context.input = near_sdk::serde_json::to_vec(&claim_args).unwrap().into();
        testing_env!(context.build());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| contract.claim_btc()));
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "Bitcoin withdrawal retry is allowed only after 60 seconds")]
    fn test_second_btc_withdrawal_is_rejected_while_one_is_pending() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 100, 100, None);
        set_test_destinations(&mut contract);
        let args = near_sdk::serde_json::json!({"txid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "vout":0,"value_sats":10000,"fee_rate_sat_vb":1});
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
        let _ = contract.withdraw_btc();
    }

    #[test]
    #[should_panic(expected = "Retry must use the same UTXO because a previously signed transaction may still be broadcast")]
    fn test_btc_withdrawal_different_utxo_retry_is_rejected_after_timeout() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000).account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), MAX_INTERVAL_SEC, MAX_INTERVAL_SEC, None);
        set_test_destinations(&mut contract);
        let args = near_sdk::serde_json::json!({"txid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "vout":0,"value_sats":10000,"fee_rate_sat_vb":1});
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
        context.block_timestamp(1_000_000_000 + RETRY_TIMEOUT_NS);
        let retry = near_sdk::serde_json::json!({"txid":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "vout":0,"value_sats":20000,"fee_rate_sat_vb":2});
        context.context.input = near_sdk::serde_json::to_vec(&retry).unwrap().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
    }

    #[test]
    #[should_panic(expected = "Estimated fee exceeds 5% of input value")]
    fn test_btc_withdrawal_enforces_fee_cap() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 100, 100, None);
        set_test_destinations(&mut contract);
        let args = near_sdk::serde_json::json!({"txid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "vout":0,"value_sats":10000,"fee_rate_sat_vb":10});
        context.context.input = near_sdk::serde_json::to_vec(&args).unwrap().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
    }

    #[test]
    #[should_panic(expected = "Withdrawals are disabled after Claimable")]
    fn test_btc_withdrawal_rejected_in_claimable() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000).account_balance(NearToken::from_yoctonear(10));
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 1, 1, None);
        set_test_destinations(&mut contract);
        context.block_timestamp(3_000_000_000);
        context.context.input = br#"{"txid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","vout":0,"value_sats":"10000","fee_rate_sat_vb":"1"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_btc();
    }

    #[test]
    #[should_panic(expected = "Vault record not found")]
    fn test_non_owner_cannot_withdraw_eth() {
        use near_sdk::test_utils::VMContextBuilder;
        use near_sdk::testing_env;
        let owner: AccountId = "owner.testnet".parse().unwrap();
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(owner.clone()).current_account_id("vault.testnet".parse().unwrap());
        testing_env!(context.build());
        let mut contract = VaultContract::new(None);
        setup_test(&mut contract, one_valid_heir(), 100, 100, None);
        set_test_destinations(&mut contract);
        context.predecessor_account_id("attacker.testnet".parse().unwrap());
        context.context.input = br#"{"balance_wei":"1000000000000000000","nonce":"0","gas_price_wei":"1000000000"}"#.to_vec().into();
        testing_env!(context.build());
        let _ = contract.withdraw_eth();
    }
}
