use near_sdk::json_types::{U128, U64};
use near_sdk::{
    env, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError,
};

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

/// Bitcoin P2WPKH Sequence: 0xfffffffd enables BIP-125 Opt-In Replace-By-Fee (RBF)
pub const BTC_P2WPKH_SEQUENCE: u32 = 0xfffffffd;

/// Max Bitcoin fee rate: 200 sat/vB
pub const MAX_BTC_FEE_RATE_SAT_VB: u64 = 200;

/// Default Bitcoin sender pubkey hash (20-byte hex).
/// Derived via NEAR MPC on path "bitcoin-1" for account "vault-heirvault01.testnet".
/// Corresponds to Native SegWit (P2WPKH) address: tb1qwlkej62grq3c2juvs595t94pvqctkdyq6qyygg
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
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: u64,
    pub last_txid: Option<String>,
    pub last_vout: Option<u32>,
    pub last_fee_rate_sat_vb: Option<u64>,
    pub confirmations: Vec<AccountId>,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct BtcSlotView {
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
    pub signed_hash: Option<String>,
    pub last_claim_timestamp_ns: U64,
    pub last_txid: Option<String>,
    pub last_vout: Option<u32>,
    pub last_fee_rate_sat_vb: Option<U64>,
    pub confirmations: Vec<AccountId>,
}

#[near(serializers = [json, borsh])]
#[derive(Clone, Debug)]
pub struct HeirSlot {
    pub heir: HeirConfig,
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
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
    pub gas_limit: U64,
    pub snapshot_balance_wei: Option<U128>,
    pub heirs: Vec<HeirSlotView>,
    pub btc_slot: BtcSlotView,
    pub btc_sender_pubkey_hash: String,
}

#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct HeirSlotView {
    pub heir: HeirConfig,
    pub status: SlotStatus,
    pub tx_hash: Option<String>,
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
    pub owner_id: AccountId,
    pub mpc_contract: AccountId,
    pub derivation_path: String,
    pub last_heartbeat_timestamp_ns: u64,
    pub heartbeat_interval_ns: u64,
    pub grace_period_ns: u64,
    pub is_configured: bool,
    pub gas_limit: u64,
    pub snapshot_balance_wei: Option<u128>,
    pub heirs: Vec<HeirSlot>,
    pub btc_slot: BtcSlot,
    pub btc_sender_pubkey_hash: String,
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
        owner_id: AccountId,
        mpc_contract: Option<AccountId>,
        derivation_path: Option<String>,
        gas_limit: Option<U64>,
        btc_sender_pubkey_hash: Option<String>,
    ) -> Self {
        Self {
            owner_id,
            mpc_contract: mpc_contract.unwrap_or_else(|| "v1.signer-prod.testnet".parse().unwrap()),
            derivation_path: derivation_path.unwrap_or_else(|| "ethereum-1".to_string()),
            last_heartbeat_timestamp_ns: env::block_timestamp(),
            heartbeat_interval_ns: 120_000_000_000,
            grace_period_ns: 60_000_000_000,
            is_configured: false,
            gas_limit: gas_limit.map(|g| g.0).unwrap_or(DEFAULT_GAS_LIMIT),
            snapshot_balance_wei: None,
            heirs: Vec::new(),
            btc_slot: BtcSlot::default(),
            btc_sender_pubkey_hash: btc_sender_pubkey_hash
                .unwrap_or_else(|| BTC_SENDER_PUBKEY_HASH.to_string()),
        }
    }

    /// Owner sets up or updates heirs and timer parameters
    pub fn setup(
        &mut self,
        heirs: Vec<HeirConfig>,
        heartbeat_interval_sec: u64,
        grace_period_sec: u64,
        gas_limit: Option<U64>,
    ) {
        self.assert_owner();

        // Block setup during Grace or Claimable
        if self.is_configured {
            let current_state = self.calculate_state();
            require!(
                current_state != VaultState::Grace && current_state != VaultState::Claimable,
                "Setup is blocked during Grace or Claimable state"
            );

            // Block setup once any slot has started (Pending, Signed, Done)
            for slot in &self.heirs {
                require!(
                    slot.status == SlotStatus::Unsent,
                    "Setup is blocked once any heir slot distribution has started"
                );
            }
            require!(
                self.btc_slot.status == SlotStatus::Unsent,
                "Setup is blocked once BTC slot distribution has started"
            );
        }

        require!(!heirs.is_empty(), "Heirs list cannot be empty");
        require!(heartbeat_interval_sec > 0, "Heartbeat interval must be > 0");
        require!(grace_period_sec > 0, "Grace period must be > 0");

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

        self.heartbeat_interval_ns = heartbeat_interval_sec * 1_000_000_000;
        self.grace_period_ns = grace_period_sec * 1_000_000_000;
        self.last_heartbeat_timestamp_ns = env::block_timestamp();
        self.is_configured = true;
        self.snapshot_balance_wei = None;
        if let Some(gl) = gas_limit {
            self.gas_limit = gl.0;
        }

        self.heirs = heirs
            .into_iter()
            .map(|heir| HeirSlot {
                heir,
                status: SlotStatus::Unsent,
                tx_hash: None,
                signed_hash: None,
                last_claim_timestamp_ns: 0,
                last_nonce: 0,
                last_gas_price_wei: U128(0),
            })
            .collect();

        self.btc_slot = BtcSlot::default();
    }

    /// Owner pings the contract to reset heartbeat timer
    pub fn ping(&mut self) {
        self.assert_owner();
        require!(self.is_configured, "Vault is not configured yet");

        let current_state = self.calculate_state();
        require!(
            current_state != VaultState::Claimable,
            "Cannot ping: vault has already reached Claimable state"
        );

        self.last_heartbeat_timestamp_ns = env::block_timestamp();
    }

    /// Claim distribution for heir's own slot.
    /// The contract builds the type 1 (EIP-2930) transfer itself:
    /// - Recipient is heir's stored address
    /// - Value is their share of the initial balance snapshot minus fee
    /// - Fixed chain ID (11155111)
    /// - Empty access list
    #[payable]
    pub fn claim(&mut self) -> Promise {
        // Enforce exact arguments (no extra arguments like tx_payload allowed)
        let args: ClaimArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");

        let heir_index = args.heir_index;
        let balance_wei = args.balance_wei;
        let nonce = args.nonce;
        let gas_price_wei = args.gas_price_wei;

        require!(self.is_configured, "Vault is not configured");
        let state = self.calculate_state();
        require!(
            state == VaultState::Claimable,
            "Vault is not claimable yet: must pass grace period"
        );

        let idx = heir_index as usize;
        require!(idx < self.heirs.len(), "Recipient index not in plan");

        let slot = &self.heirs[idx];
        let caller = env::predecessor_account_id();

        // Only the heir can claim their own slot
        require!(
            caller == slot.heir.near_account_id,
            "Only the designated heir can claim this slot"
        );

        // Snapshot balance at first claim so every heir gets their equal share of the original total pool
        let total_pool_wei = match self.snapshot_balance_wei {
            Some(snap) => snap,
            None => {
                self.snapshot_balance_wei = Some(balance_wei.0);
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
        let fee_wei = (self.gas_limit as u128)
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
            self.gas_limit,
            recipient_bytes,
            transfer_value,
        );

        // Keccak256 hash
        let tx_hash_bytes = env::keccak256_array(&unsigned_tx_bytes);
        let tx_hash_hex = hex_string(&tx_hash_bytes);

        // Update slot state
        let slot_mut = &mut self.heirs[idx];
        slot_mut.status = SlotStatus::Pending;
        slot_mut.signed_hash = Some(tx_hash_hex);
        slot_mut.last_claim_timestamp_ns = now_ns;
        slot_mut.last_nonce = nonce.0;
        slot_mut.last_gas_price_wei = gas_price_wei;

        let sign_request = SignRequest {
            payload: tx_hash_bytes,
            path: self.derivation_path.clone(),
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
            near_sdk::serde_json::to_vec(&serde_json_callback_args(heir_index))
                .expect("Failed to serialize callback args"),
            NearToken::from_yoctonear(0),
            CALLBACK_GAS,
        );

        mpc_promise.then(callback_promise)
    }

    /// Callback invoked after MPC sign call finishes
    #[private]
    pub fn on_claim_signed(
        &mut self,
        heir_index: u32,
        #[callback_result] call_result: Result<near_sdk::serde_json::Value, PromiseError>,
    ) -> bool {
        let idx = heir_index as usize;
        if idx >= self.heirs.len() {
            return false;
        }

        match call_result {
            Ok(signature_value) => {
                // MPC signed successfully -> Slot state becomes Signed
                self.heirs[idx].status = SlotStatus::Signed;
                self.heirs[idx].tx_hash = Some(signature_value.to_string());
                true
            }
            Err(_) => {
                // MPC call failed or rejected -> Revert to Unsent so it can be retried immediately
                self.heirs[idx].status = SlotStatus::Unsent;
                false
            }
        }
    }

    /// Heir marks slot as Done after confirming on-chain receipt on Sepolia
    pub fn mark_done(&mut self, heir_index: u32) {
        let idx = heir_index as usize;
        require!(idx < self.heirs.len(), "Recipient index not in plan");

        let slot = &mut self.heirs[idx];
        let caller = env::predecessor_account_id();

        require!(
            caller == slot.heir.near_account_id,
            "Only the designated heir can mark this slot as Done"
        );
        require!(
            slot.status == SlotStatus::Signed,
            "Slot must be in Signed state to be marked as Done"
        );

        slot.status = SlotStatus::Done;
    }

    /// Claim Bitcoin distribution.
    /// Only an heir can call it, only when Claimable, with deny_unknown_fields.
    /// Arguments: one UTXO (txid, vout, value_sats) and fee_rate_sat_vb. Exactly one input.
    /// The contract builds the outputs itself: for each heir, (input minus fee) times their bps,
    /// the last heir takes the remainder; fee = 141 vB times the fee rate, with a rate cap (<= 200 sat/vB)
    /// and a 5% of input cap; reject any output under 294 sats; reject if any heir lacks a Bitcoin program.
    /// Uses sequence 0xfffffffd, computes sighash, calls MPC sign with path bitcoin-1.
    #[payable]
    pub fn claim_btc(&mut self) -> Promise {
        let args: ClaimBtcArgs = near_sdk::serde_json::from_slice(&env::input().unwrap_or_default())
            .expect("Failed to deserialize input arguments or unexpected extra argument provided");

        require!(self.is_configured, "Vault is not configured");
        let state = self.calculate_state();
        require!(
            state == VaultState::Claimable,
            "Vault is not claimable yet: must pass grace period"
        );

        let caller = env::predecessor_account_id();
        require!(
            self.heirs.iter().any(|h| h.heir.near_account_id == caller),
            "Only a designated heir can call claim_btc"
        );

        let now_ns = env::block_timestamp();

        // BTC Slot state checks & retry conditions
        match self.btc_slot.status {
            SlotStatus::Unsent => {
                // First claim attempt allowed
            }
            SlotStatus::Pending | SlotStatus::Signed => {
                // Allow retry only after 60s timeout
                let elapsed = now_ns.saturating_sub(self.btc_slot.last_claim_timestamp_ns);
                require!(
                    elapsed >= RETRY_TIMEOUT_NS,
                    "Retry is allowed only after 60 seconds timeout"
                );
                // Same txid and vout needs a strictly higher fee rate; a different UTXO is allowed after timeout with no fee condition
                if let (Some(prev_txid), Some(prev_vout)) = (&self.btc_slot.last_txid, self.btc_slot.last_vout) {
                    if args.txid == *prev_txid && args.vout == prev_vout {
                        if let Some(prev_fee_rate) = self.btc_slot.last_fee_rate_sat_vb {
                            require!(
                                args.fee_rate_sat_vb.0 > prev_fee_rate,
                                "Retry with same UTXO must provide a strictly higher fee rate than the previous attempt"
                            );
                        }
                    }
                }
            }
            SlotStatus::Done => {
                env::panic_str("BTC slot is already Done");
            }
        }

        // Fee calculation, caps, dust check, and output calculation via pure function
        let fee_rate = args.fee_rate_sat_vb.0;
        let input_value = args.value_sats.0;

        let heirs_config: Vec<HeirConfig> = self.heirs.iter().map(|h| h.heir.clone()).collect();
        let outputs = calculate_btc_outputs(input_value, fee_rate, &heirs_config)
            .unwrap_or_else(|err| env::panic_str(&err));

        let txid_bytes = hex_to_bytes32(&args.txid);
        let sender_pubkey_hash = hex_to_bytes20(&self.btc_sender_pubkey_hash);

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
        let previous_status = self.btc_slot.status;
        self.btc_slot.status = SlotStatus::Pending;
        self.btc_slot.last_claim_timestamp_ns = now_ns;

        let sign_request = SignRequest {
            payload: sighash,
            path: "bitcoin-1".to_string(),
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

        mpc_promise.then(callback_promise)
    }

    #[private]
    pub fn on_btc_claim_signed(
        &mut self,
        sighash_hex: String,
        txid: String,
        vout: u32,
        fee_rate_sat_vb: U64,
        previous_status: SlotStatus,
        claim_timestamp_ns: Option<U64>,
        #[callback_result] call_result: Result<near_sdk::serde_json::Value, PromiseError>,
    ) -> bool {
        // A failed or late callback from an older attempt does not overwrite a newer attempt
        if let Some(ts) = claim_timestamp_ns {
            if ts.0 < self.btc_slot.last_claim_timestamp_ns {
                return false;
            }
        }
        if self.btc_slot.status == SlotStatus::Done {
            return false;
        }

        match call_result {
            Ok(signature_value) => {
                // MPC signed successfully -> commit hash, signature, UTXO, and fee rate together
                self.btc_slot.status = SlotStatus::Signed;
                self.btc_slot.signed_hash = Some(sighash_hex);
                self.btc_slot.tx_hash = Some(signature_value.to_string());
                self.btc_slot.last_txid = Some(txid);
                self.btc_slot.last_vout = Some(vout);
                self.btc_slot.last_fee_rate_sat_vb = Some(fee_rate_sat_vb.0);
                self.btc_slot.confirmations.clear();
                true
            }
            Err(_) => {
                // A failed retry must keep the previous Signed state
                if previous_status == SlotStatus::Signed {
                    self.btc_slot.status = SlotStatus::Signed;
                } else {
                    self.btc_slot.status = SlotStatus::Unsent;
                }
                false
            }
        }
    }

    /// Heir marks BTC slot as confirmed received.
    /// Requires confirmation from every heir before transitioning to Done.
    pub fn mark_btc_done(&mut self) {
        let caller = env::predecessor_account_id();
        require!(
            self.heirs.iter().any(|h| h.heir.near_account_id == caller),
            "Only a designated heir can mark BTC slot as Done"
        );
        require!(
            self.btc_slot.status != SlotStatus::Done,
            "BTC slot is already Done"
        );
        require!(
            self.btc_slot.status == SlotStatus::Signed,
            "BTC slot must be in Signed state to be marked as Done"
        );

        if !self.btc_slot.confirmations.contains(&caller) {
            self.btc_slot.confirmations.push(caller);
        }

        let all_confirmed = self
            .heirs
            .iter()
            .all(|h| self.btc_slot.confirmations.contains(&h.heir.near_account_id));

        if all_confirmed {
            self.btc_slot.status = SlotStatus::Done;
        }
    }

    /// Get current state of the vault and timers
    pub fn status(&self) -> VaultStatusView {
        let current_ns = env::block_timestamp();
        let state = self.calculate_state();
        let elapsed_ns = current_ns.saturating_sub(self.last_heartbeat_timestamp_ns);

        let warning_threshold_ns = self.heartbeat_interval_ns * 3 / 4;
        let grace_start_ns = self.heartbeat_interval_ns;
        let claimable_start_ns = self.heartbeat_interval_ns + self.grace_period_ns;

        let seconds_since_heartbeat = elapsed_ns / 1_000_000_000;
        let seconds_until_grace = grace_start_ns.saturating_sub(elapsed_ns) / 1_000_000_000;
        let seconds_until_claimable = claimable_start_ns.saturating_sub(elapsed_ns) / 1_000_000_000;

        VaultStatusView {
            state,
            owner_id: self.owner_id.clone(),
            last_heartbeat_timestamp_ns: U64(self.last_heartbeat_timestamp_ns),
            current_timestamp_ns: U64(current_ns),
            heartbeat_interval_sec: U64(self.heartbeat_interval_ns / 1_000_000_000),
            grace_period_sec: U64(self.grace_period_ns / 1_000_000_000),
            warning_threshold_sec: U64(warning_threshold_ns / 1_000_000_000),
            seconds_since_heartbeat: U64(seconds_since_heartbeat),
            seconds_until_grace: U64(seconds_until_grace),
            seconds_until_claimable: U64(seconds_until_claimable),
            mpc_contract: self.mpc_contract.clone(),
            derivation_path: self.derivation_path.clone(),
            gas_limit: U64(self.gas_limit),
            snapshot_balance_wei: self.snapshot_balance_wei.map(U128),
            heirs: self
                .heirs
                .iter()
                .map(|h| HeirSlotView {
                    heir: h.heir.clone(),
                    status: h.status,
                    tx_hash: h.tx_hash.clone(),
                    signed_hash: h.signed_hash.clone(),
                    last_claim_timestamp_ns: U64(h.last_claim_timestamp_ns),
                    last_nonce: U64(h.last_nonce),
                    last_gas_price_wei: h.last_gas_price_wei,
                })
                .collect(),
            btc_slot: BtcSlotView {
                status: self.btc_slot.status,
                tx_hash: self.btc_slot.tx_hash.clone(),
                signed_hash: self.btc_slot.signed_hash.clone(),
                last_claim_timestamp_ns: U64(self.btc_slot.last_claim_timestamp_ns),
                last_txid: self.btc_slot.last_txid.clone(),
                last_vout: self.btc_slot.last_vout,
                last_fee_rate_sat_vb: self.btc_slot.last_fee_rate_sat_vb.map(U64),
                confirmations: self.btc_slot.confirmations.clone(),
            },
            btc_sender_pubkey_hash: self.btc_sender_pubkey_hash.clone(),
        }
    }

    // Helper functions
    fn assert_owner(&self) {
        require!(
            env::predecessor_account_id() == self.owner_id,
            "Only owner can call this method"
        );
    }

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

pub fn hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
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

fn serde_json_callback_args(heir_index: u32) -> near_sdk::serde_json::Value {
    near_sdk::serde_json::json!({
        "heir_index": heir_index
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[should_panic(expected = "Only a designated heir can call claim_btc")]
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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
    #[should_panic(expected = "Only a designated heir can mark BTC slot as Done")]
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;

        context.predecessor_account_id(stranger);
        testing_env!(context.build());

        contract.mark_btc_done();
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;

        // Heir 1 confirms: should remain Signed
        context.predecessor_account_id(heir1.clone());
        testing_env!(context.build());
        contract.mark_btc_done();
        assert_eq!(contract.btc_slot.status, SlotStatus::Signed);
        assert_eq!(contract.btc_slot.confirmations.len(), 1);

        // Heir 2 confirms: all confirmed -> becomes Done
        context.predecessor_account_id(heir2);
        testing_env!(context.build());
        contract.mark_btc_done();
        assert_eq!(contract.btc_slot.status, SlotStatus::Done);
        assert_eq!(contract.btc_slot.confirmations.len(), 2);
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Done;

        context.predecessor_account_id(heir1);
        testing_env!(context.build());

        // Calling mark_btc_done when already Done triggers Done lock
        contract.mark_btc_done();
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;
        contract.btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        contract.btc_slot.last_vout = Some(1);
        contract.btc_slot.last_fee_rate_sat_vb = Some(10);
        contract.btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance 61 seconds (eligible for retry timeout)
        let claim_args = ClaimBtcArgs {
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
    fn test_claim_btc_retry_different_utxo_allowed() {
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;
        contract.btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        contract.btc_slot.last_vout = Some(1);
        contract.btc_slot.last_fee_rate_sat_vb = Some(10);
        contract.btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance 61 seconds (past 60s timeout); retrying with a different UTXO has no fee condition (even fee_rate 5 < 10)
        let claim_args = ClaimBtcArgs {
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
        assert_eq!(contract.btc_slot.status, SlotStatus::Pending);
        assert_eq!(contract.btc_slot.last_claim_timestamp_ns, 62_000_000_000);
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;
        contract.btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        contract.btc_slot.last_vout = Some(1);
        contract.btc_slot.last_fee_rate_sat_vb = Some(10);
        contract.btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance 61 seconds (past 60s timeout); retry with same UTXO and strictly higher fee rate (15 > 10) succeeds
        let claim_args = ClaimBtcArgs {
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
        assert_eq!(contract.btc_slot.status, SlotStatus::Pending);
        assert_eq!(contract.btc_slot.last_claim_timestamp_ns, 62_000_000_000);
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;
        contract.btc_slot.last_txid = Some("781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string());
        contract.btc_slot.last_vout = Some(1);
        contract.btc_slot.last_fee_rate_sat_vb = Some(10);
        contract.btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance only 30 seconds (< 60s timeout); retry is rejected
        let claim_args = ClaimBtcArgs {
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        contract.btc_slot.status = SlotStatus::Signed;

        // Heir 1 confirms first time
        context.predecessor_account_id(heir1.clone());
        testing_env!(context.build());
        contract.mark_btc_done();

        assert_eq!(contract.btc_slot.confirmations.len(), 1);
        assert_eq!(contract.btc_slot.status, SlotStatus::Signed);

        // Heir 1 confirms second time - counts once, does not duplicate, status remains Signed
        contract.mark_btc_done();

        assert_eq!(contract.btc_slot.confirmations.len(), 1);
        assert_eq!(contract.btc_slot.confirmations[0], heir1);
        assert_eq!(contract.btc_slot.status, SlotStatus::Signed);
    }

    #[test]
    fn test_claim_btc_retry_while_pending_no_stored_values_allowed() {
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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

        // BTC slot is Pending, but callback hasn't returned yet, so no stored txid, vout, or fee rate
        contract.btc_slot.status = SlotStatus::Pending;
        contract.btc_slot.last_txid = None;
        contract.btc_slot.last_vout = None;
        contract.btc_slot.last_fee_rate_sat_vb = None;
        contract.btc_slot.last_claim_timestamp_ns = 1_000_000_000;

        // Advance past 60s timeout (e.g. 62s)
        let claim_args = ClaimBtcArgs {
            txid: "781d7071c41d1177a6364d8f7c3ed5c44590882907796e30e4324dc0b2945a72".to_string(),
            vout: 1,
            value_sats: U64(122462),
            fee_rate_sat_vb: U64(10),
        };
        let input_bytes = near_sdk::serde_json::to_vec(&claim_args).unwrap();

        context
            .predecessor_account_id(heir1)
            .block_timestamp(62_000_000_000);
        context.context.input = input_bytes.into();
        testing_env!(context.build());

        // A retry while Pending with no stored values neither panics nor locks the slot
        let _ = contract.claim_btc();
        assert_eq!(contract.btc_slot.status, SlotStatus::Pending);
        assert_eq!(contract.btc_slot.last_claim_timestamp_ns, 62_000_000_000);
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

        let mut contract = VaultContract::new(owner.clone(), None, None, None, None);
        contract.setup(
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
        contract.btc_slot.status = SlotStatus::Pending;
        contract.btc_slot.last_claim_timestamp_ns = 70_000_000_000;

        // Late failed callback arrives from Attempt 1 (timestamp t = 1_000_000_000)
        let res_err = contract.on_btc_claim_signed(
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
        assert_eq!(contract.btc_slot.status, SlotStatus::Pending);

        // Attempt 2 succeeds and commits Signed state
        contract.btc_slot.status = SlotStatus::Signed;
        contract.btc_slot.signed_hash = Some("new_hash".to_string());
        contract.btc_slot.tx_hash = Some("new_sig".to_string());
        contract.btc_slot.last_txid = Some("new_txid".to_string());
        contract.btc_slot.last_vout = Some(1);
        contract.btc_slot.last_fee_rate_sat_vb = Some(20);

        // Another late callback from Attempt 1 arrives with Ok
        let res_late_ok = contract.on_btc_claim_signed(
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
        assert_eq!(contract.btc_slot.status, SlotStatus::Signed);
        assert_eq!(contract.btc_slot.signed_hash.as_deref(), Some("new_hash"));
        assert_eq!(contract.btc_slot.tx_hash.as_deref(), Some("new_sig"));
        assert_eq!(contract.btc_slot.last_txid.as_deref(), Some("new_txid"));
        assert_eq!(contract.btc_slot.last_vout, Some(1));
        assert_eq!(contract.btc_slot.last_fee_rate_sat_vb, Some(20));
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
}
