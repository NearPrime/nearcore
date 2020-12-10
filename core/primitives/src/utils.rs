use std::cmp::max;
use std::convert::{AsRef, TryFrom};
use std::fmt;

use byteorder::{LittleEndian, WriteBytesExt};
use chrono::{DateTime, NaiveDateTime, Utc};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use regex::Regex;
use serde;

use lazy_static::lazy_static;

use crate::hash::{hash, CryptoHash};
use crate::receipt::Receipt;
use crate::transaction::SignedTransaction;
use crate::types::{AccountId, CompiledContractCache, NumSeats, NumShards, ShardId};
use crate::version::{
    ProtocolVersion, CORRECT_RANDOM_VALUE_PROTOCOL_VERSION, CREATE_HASH_PROTOCOL_VERSION,
};
use std::mem::size_of;
use std::ops::Deref;

pub const MIN_ACCOUNT_ID_LEN: usize = 2;
pub const MAX_ACCOUNT_ID_LEN: usize = 64;

/// Number of nano seconds in a second.
const NS_IN_SECOND: u64 = 1_000_000_000;

/// A data structure for tagging data as already being validated to prevent redundant work.
#[derive(Debug)]
pub enum MaybeValidated<T> {
    Validated(T),
    NotValidated(T),
}

impl<T> MaybeValidated<T> {
    pub fn validate_with<E, F: FnOnce(&T) -> Result<bool, E>>(&self, f: F) -> Result<bool, E> {
        match &self {
            Self::Validated(_) => Ok(true),
            Self::NotValidated(t) => f(t),
        }
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> MaybeValidated<U> {
        match self {
            Self::Validated(t) => MaybeValidated::Validated(f(t)),
            Self::NotValidated(t) => MaybeValidated::NotValidated(f(t)),
        }
    }

    pub fn as_ref(&self) -> MaybeValidated<&T> {
        match &self {
            Self::Validated(ref t) => MaybeValidated::Validated(t),
            Self::NotValidated(ref t) => MaybeValidated::NotValidated(t),
        }
    }

    pub fn extract(self) -> T {
        match self {
            Self::Validated(t) => t,
            Self::NotValidated(t) => t,
        }
    }
}

impl<T: Sized> Deref for MaybeValidated<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self {
            Self::Validated(t) => t,
            Self::NotValidated(t) => t,
        }
    }
}

pub fn get_block_shard_id(block_hash: &CryptoHash, shard_id: ShardId) -> Vec<u8> {
    let mut res = Vec::with_capacity(40);
    res.extend_from_slice(block_hash.as_ref());
    res.extend_from_slice(&shard_id.to_le_bytes());
    res
}

pub fn get_block_shard_id_rev(
    key: &[u8],
) -> Result<(CryptoHash, ShardId), Box<dyn std::error::Error>> {
    if key.len() != 40 {
        return Err(
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid key length").into()
        );
    }
    let block_hash_vec: Vec<u8> = key[0..32].iter().cloned().collect();
    let block_hash = CryptoHash::try_from(block_hash_vec)?;
    let mut shard_id_arr: [u8; 8] = Default::default();
    shard_id_arr.copy_from_slice(&key[key.len() - 8..]);
    let shard_id = ShardId::from_le_bytes(shard_id_arr);
    Ok((block_hash, shard_id))
}

/// Creates a new Receipt ID from a given signed transaction and a block hash.
/// This method is backward compatible, so it takes the current protocol version.
pub fn create_receipt_id_from_transaction(
    protocol_version: ProtocolVersion,
    signed_transaction: &SignedTransaction,
    block_hash: &CryptoHash,
) -> CryptoHash {
    create_hash_upgradable(protocol_version, &signed_transaction.get_hash(), &block_hash, 0)
}

/// Creates a new Receipt ID from a given receipt, a block hash and a new receipt index.
/// This method is backward compatible, so it takes the current protocol version.
pub fn create_receipt_id_from_receipt(
    protocol_version: ProtocolVersion,
    receipt: &Receipt,
    block_hash: &CryptoHash,
    receipt_index: usize,
) -> CryptoHash {
    create_hash_upgradable(protocol_version, &receipt.receipt_id, &block_hash, receipt_index as u64)
}

/// Creates a new action_hash from a given receipt, a block hash and an action index.
/// This method is backward compatible, so it takes the current protocol version.
pub fn create_action_hash(
    protocol_version: ProtocolVersion,
    receipt: &Receipt,
    block_hash: &CryptoHash,
    action_index: usize,
) -> CryptoHash {
    // Action hash uses the same input as a new receipt ID, so to avoid hash conflicts we use the
    // salt starting from the `u64` going backward.
    let salt = u64::max_value() - action_index as u64;
    create_hash_upgradable(protocol_version, &receipt.receipt_id, &block_hash, salt)
}

/// Creates a new `data_id` from a given action hash, a block hash and a data index.
/// This method is backward compatible, so it takes the current protocol version.
pub fn create_data_id(
    protocol_version: ProtocolVersion,
    action_hash: &CryptoHash,
    block_hash: &CryptoHash,
    data_index: usize,
) -> CryptoHash {
    create_hash_upgradable(protocol_version, &action_hash, &block_hash, data_index as u64)
}

/// Creates a unique random seed to be provided to `VMContext` from a give `action_hash` and
/// a given `random_seed`.
/// This method is backward compatible, so it takes the current protocol version.
pub fn create_random_seed(
    protocol_version: ProtocolVersion,
    action_hash: CryptoHash,
    random_seed: CryptoHash,
) -> Vec<u8> {
    let res = if protocol_version < CORRECT_RANDOM_VALUE_PROTOCOL_VERSION {
        action_hash
    } else if protocol_version < CREATE_HASH_PROTOCOL_VERSION {
        random_seed
    } else {
        // Generates random seed from random_seed and action_hash.
        // Since every action hash is unique, the seed will be unique per receipt and even
        // per action within a receipt.
        let mut bytes: Vec<u8> =
            Vec::with_capacity(size_of::<CryptoHash>() + size_of::<CryptoHash>());
        bytes.extend_from_slice(action_hash.as_ref());
        bytes.extend_from_slice(random_seed.as_ref());
        hash(&bytes)
    };
    res.as_ref().to_vec()
}

/// Creates a new CryptoHash ID based on the protocol version.
/// Before `CREATE_HASH_PROTOCOL_VERSION` it uses `create_nonce_with_nonce` with
/// just `base` and `salt`. But after `CREATE_HASH_PROTOCOL_VERSION` it uses
/// `extra_hash` in addition to the `base` and `salt`.
/// E.g. this `extra_hash` can be a block hash to distinguish receipts between forks.
fn create_hash_upgradable(
    protocol_version: ProtocolVersion,
    base: &CryptoHash,
    extra_hash: &CryptoHash,
    salt: u64,
) -> CryptoHash {
    if protocol_version < CREATE_HASH_PROTOCOL_VERSION {
        create_nonce_with_nonce(base, salt)
    } else {
        let mut bytes: Vec<u8> = Vec::with_capacity(
            size_of::<CryptoHash>() + size_of::<CryptoHash>() + size_of::<u64>(),
        );
        bytes.extend_from_slice(base.as_ref());
        bytes.extend_from_slice(extra_hash.as_ref());
        bytes.extend(index_to_bytes(salt));
        hash(&bytes)
    }
}

/// Deprecated. Please use `create_hash_upgradable`
fn create_nonce_with_nonce(base: &CryptoHash, salt: u64) -> CryptoHash {
    let mut nonce: Vec<u8> = base.as_ref().to_owned();
    nonce.extend(index_to_bytes(salt));
    hash(&nonce)
}

pub fn index_to_bytes(index: u64) -> Vec<u8> {
    let mut bytes = vec![];
    bytes.write_u64::<LittleEndian>(index).expect("writing to bytes failed");
    bytes
}

lazy_static! {
    /// See NEP#0006
    static ref VALID_ACCOUNT_ID: Regex =
        Regex::new(r"^(([a-z\d]+[\-_])*[a-z\d]+\.)*([a-z\d]+[\-_])*[a-z\d]+$").unwrap();
    /// Represents a part of an account ID with a suffix of as a separator `.`.
    static ref VALID_ACCOUNT_PART_ID_WITH_TAIL_SEPARATOR: Regex =
        Regex::new(r"^([a-z\d]+[\-_])*[a-z\d]+\.$").unwrap();
    /// Represents a top level account ID.
    static ref VALID_TOP_LEVEL_ACCOUNT_ID: Regex =
        Regex::new(r"^([a-z\d]+[\-_])*[a-z\d]+$").unwrap();
}

/// const does not allow function call, so have to resort to this
pub fn system_account() -> AccountId {
    "system".to_string()
}

pub fn is_valid_account_id(account_id: &AccountId) -> bool {
    account_id.len() >= MIN_ACCOUNT_ID_LEN
        && account_id.len() <= MAX_ACCOUNT_ID_LEN
        && VALID_ACCOUNT_ID.is_match(account_id)
}

pub fn is_valid_top_level_account_id(account_id: &AccountId) -> bool {
    account_id.len() >= MIN_ACCOUNT_ID_LEN
        && account_id.len() <= MAX_ACCOUNT_ID_LEN
        && account_id != &system_account()
        && VALID_TOP_LEVEL_ACCOUNT_ID.is_match(account_id)
}

/// Returns true if the signer_id can create a direct sub-account with the given account Id.
/// It assumes the signer_id is a valid account_id
pub fn is_valid_sub_account_id(signer_id: &AccountId, sub_account_id: &AccountId) -> bool {
    if !is_valid_account_id(sub_account_id) {
        return false;
    }
    if signer_id.len() >= sub_account_id.len() {
        return false;
    }
    // Will not panic, since valid account id is utf-8 only and the length is checked above.
    // e.g. when `near` creates `aa.near`, it splits into `aa.` and `near`
    let (prefix, suffix) = sub_account_id.split_at(sub_account_id.len() - signer_id.len());
    if suffix != signer_id {
        return false;
    }
    VALID_ACCOUNT_PART_ID_WITH_TAIL_SEPARATOR.is_match(prefix)
}

/// A wrapper around Option<T> that provides native Display trait.
/// Simplifies propagating automatic Display trait on parent structs.
pub struct DisplayOption<T>(pub Option<T>);

impl<T: fmt::Display> fmt::Display for DisplayOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(ref v) => write!(f, "Some({})", v),
            None => write!(f, "None"),
        }
    }
}

impl<T> DisplayOption<T> {
    pub fn into(self) -> Option<T> {
        self.0
    }
}

impl<T> AsRef<Option<T>> for DisplayOption<T> {
    fn as_ref(&self) -> &Option<T> {
        &self.0
    }
}

impl<T: fmt::Display> From<Option<T>> for DisplayOption<T> {
    fn from(o: Option<T>) -> Self {
        DisplayOption(o)
    }
}

/// Macro to either return value if the result is Ok, or exit function logging error.
#[macro_export]
macro_rules! unwrap_or_return {
    ($obj: expr, $ret: expr) => {
        match $obj {
            Ok(value) => value,
            Err(err) => {
                error!(target: "client", "Unwrap error: {}", err);
                return $ret;
            }
        }
    };
    ($obj: expr) => {
        match $obj {
            Ok(value) => value,
            Err(err) => {
                error!(target: "client", "Unwrap error: {}", err);
                return;
            }
        }
    };
}

/// Macro to either return value if the result is Some, or exit function.
#[macro_export]
macro_rules! unwrap_option_or_return {
    ($obj: expr, $ret: expr) => {
        match $obj {
            Some(value) => value,
            None => {
                return $ret;
            }
        }
    };
    ($obj: expr) => {
        match $obj {
            Some(value) => value,
            None => {
                return;
            }
        }
    };
}

/// Converts timestamp in ns into DateTime UTC time.
pub fn from_timestamp(timestamp: u64) -> DateTime<Utc> {
    DateTime::from_utc(
        NaiveDateTime::from_timestamp(
            (timestamp / NS_IN_SECOND) as i64,
            (timestamp % NS_IN_SECOND) as u32,
        ),
        Utc,
    )
}

/// Converts DateTime UTC time into timestamp in ns.
pub fn to_timestamp(time: DateTime<Utc>) -> u64 {
    time.timestamp_nanos() as u64
}

/// Compute number of seats per shard for given total number of seats and number of shards.
pub fn get_num_seats_per_shard(num_shards: NumShards, num_seats: NumSeats) -> Vec<NumSeats> {
    (0..num_shards)
        .map(|i| {
            let remainder = num_seats % num_shards;
            let num = if i < remainder as u64 {
                num_seats / num_shards + 1
            } else {
                num_seats / num_shards
            };
            max(num, 1)
        })
        .collect()
}

/// Generate random string of given length
pub fn generate_random_string(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).collect::<String>()
}

pub struct Serializable<'a, T>(&'a T);

impl<'a, T> fmt::Display for Serializable<'a, T>
where
    T: serde::Serialize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", serde_json::to_string(&self.0).unwrap())
    }
}

impl fmt::Debug for dyn CompiledContractCache {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Compiled contracts cache")
    }
}

/// Wrap an object that implements Serialize into another object
/// that implements Display. When used display in this object
/// it shows its json representation. It is used to display complex
/// objects using tracing.
///
/// tracing::debug!(target: "diagnostic", value=%ser(&object));
pub fn ser<'a, T>(object: &'a T) -> Serializable<'a, T>
where
    T: serde::Serialize,
{
    Serializable(object)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK_ACCOUNT_IDS: &[&str] = &[
        "aa",
        "a-a",
        "a-aa",
        "100",
        "0o",
        "com",
        "near",
        "bowen",
        "b-o_w_e-n",
        "b.owen",
        "bro.wen",
        "a.ha",
        "a.b-a.ra",
        "system",
        "over.9000",
        "google.com",
        "illia.cheapaccounts.near",
        "0o0ooo00oo00o",
        "alex-skidanov",
        "10-4.8-2",
        "b-o_w_e-n",
        "no_lols",
        "0123456789012345678901234567890123456789012345678901234567890123",
        // Valid, but can't be created
        "near.a",
    ];

    #[test]
    fn test_is_valid_account_id() {
        for account_id in OK_ACCOUNT_IDS {
            assert!(
                is_valid_account_id(&account_id.to_string()),
                "Valid account id {:?} marked invalid",
                account_id
            );
        }

        let bad_account_ids = vec![
            "a",
            "A",
            "Abc",
            "-near",
            "near-",
            "-near-",
            "near.",
            ".near",
            "near@",
            "@near",
            "неар",
            "@@@@@",
            "0__0",
            "0_-_0",
            "0_-_0",
            "..",
            "a..near",
            "nEar",
            "_bowen",
            "hello world",
            "abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz",
            "01234567890123456789012345678901234567890123456789012345678901234",
            // `@` separators are banned now
            "some-complex-address@gmail.com",
            "sub.buy_d1gitz@atata@b0-rg.c_0_m",
        ];
        for account_id in bad_account_ids {
            assert!(
                !is_valid_account_id(&account_id.to_string()),
                "Invalid account id {:?} marked valid",
                account_id
            );
        }
    }

    #[test]
    fn test_is_valid_top_level_account_id() {
        let ok_top_level_account_ids = vec![
            "aa",
            "a-a",
            "a-aa",
            "100",
            "0o",
            "com",
            "near",
            "bowen",
            "b-o_w_e-n",
            "0o0ooo00oo00o",
            "alex-skidanov",
            "b-o_w_e-n",
            "no_lols",
            "0123456789012345678901234567890123456789012345678901234567890123",
        ];
        for account_id in ok_top_level_account_ids {
            assert!(
                is_valid_top_level_account_id(&account_id.to_string()),
                "Valid top level account id {:?} marked invalid",
                account_id
            );
        }

        let bad_top_level_account_ids = vec![
            "near.a",
            "b.owen",
            "bro.wen",
            "a.ha",
            "a.b-a.ra",
            "some-complex-address@gmail.com",
            "sub.buy_d1gitz@atata@b0-rg.c_0_m",
            "over.9000",
            "google.com",
            "illia.cheapaccounts.near",
            "10-4.8-2",
            "a",
            "A",
            "Abc",
            "-near",
            "near-",
            "-near-",
            "near.",
            ".near",
            "near@",
            "@near",
            "неар",
            "@@@@@",
            "0__0",
            "0_-_0",
            "0_-_0",
            "..",
            "a..near",
            "nEar",
            "_bowen",
            "hello world",
            "abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz.abcdefghijklmnopqrstuvwxyz",
            "01234567890123456789012345678901234567890123456789012345678901234",
            // Valid regex and length, but reserved
            "system",
        ];
        for account_id in bad_top_level_account_ids {
            assert!(
                !is_valid_top_level_account_id(&account_id.to_string()),
                "Invalid top level account id {:?} marked valid",
                account_id
            );
        }
    }

    #[test]
    fn test_is_valid_sub_account_id() {
        let ok_pairs = vec![
            ("test", "a.test"),
            ("test-me", "abc.test-me"),
            ("gmail.com", "abc.gmail.com"),
            ("gmail.com", "abc-lol.gmail.com"),
            ("gmail.com", "abc_lol.gmail.com"),
            ("gmail.com", "bro-abc_lol.gmail.com"),
            ("g0", "0g.g0"),
            ("1g", "1g.1g"),
            ("5-3", "4_2.5-3"),
        ];
        for (signer_id, sub_account_id) in ok_pairs {
            assert!(
                is_valid_sub_account_id(&signer_id.to_string(), &sub_account_id.to_string()),
                "Failed to create sub-account {:?} by account {:?}",
                sub_account_id,
                signer_id
            );
        }

        let bad_pairs = vec![
            ("test", ".test"),
            ("test", "test"),
            ("test", "est"),
            ("test", ""),
            ("test", "st"),
            ("test5", "ббб"),
            ("test", "a-test"),
            ("test", "etest"),
            ("test", "a.etest"),
            ("test", "retest"),
            ("test-me", "abc-.test-me"),
            ("test-me", "Abc.test-me"),
            ("test-me", "-abc.test-me"),
            ("test-me", "a--c.test-me"),
            ("test-me", "a_-c.test-me"),
            ("test-me", "a-_c.test-me"),
            ("test-me", "_abc.test-me"),
            ("test-me", "abc_.test-me"),
            ("test-me", "..test-me"),
            ("test-me", "a..test-me"),
            ("gmail.com", "a.abc@gmail.com"),
            ("gmail.com", ".abc@gmail.com"),
            ("gmail.com", ".abc@gmail@com"),
            ("gmail.com", "abc@gmail@com"),
            ("test", "a@test"),
            ("test_me", "abc@test_me"),
            ("gmail.com", "abc@gmail.com"),
            ("gmail@com", "abc.gmail@com"),
            ("gmail.com", "abc-lol@gmail.com"),
            ("gmail@com", "abc_lol.gmail@com"),
            ("gmail@com", "bro-abc_lol.gmail@com"),
            ("gmail.com", "123456789012345678901234567890123456789012345678901234567890@gmail.com"),
            (
                "123456789012345678901234567890123456789012345678901234567890",
                "1234567890.123456789012345678901234567890123456789012345678901234567890",
            ),
            ("aa", "ъ@aa"),
            ("aa", "ъ.aa"),
        ];
        for (signer_id, sub_account_id) in bad_pairs {
            assert!(
                !is_valid_sub_account_id(&signer_id.to_string(), &sub_account_id.to_string()),
                "Invalid sub-account {:?} created by account {:?}",
                sub_account_id,
                signer_id
            );
        }
    }

    #[test]
    fn test_num_chunk_producers() {
        for num_seats in 1..50 {
            for num_shards in 1..50 {
                let assignment = get_num_seats_per_shard(num_shards, num_seats);
                assert_eq!(assignment.iter().sum::<u64>(), max(num_seats, num_shards));
            }
        }
    }

    #[test]
    fn test_create_hash_upgradable() {
        let base = hash(b"atata");
        let extra_base = hash(b"hohoho");
        let other_extra_base = hash(b"banana");
        let salt = 3;
        assert_eq!(
            create_nonce_with_nonce(&base, salt),
            create_hash_upgradable(CREATE_HASH_PROTOCOL_VERSION - 1, &base, &extra_base, salt)
        );
        assert_ne!(
            create_nonce_with_nonce(&base, salt),
            create_hash_upgradable(CREATE_HASH_PROTOCOL_VERSION, &base, &extra_base, salt)
        );
        assert_ne!(
            create_hash_upgradable(CREATE_HASH_PROTOCOL_VERSION, &base, &extra_base, salt),
            create_hash_upgradable(CREATE_HASH_PROTOCOL_VERSION, &base, &other_extra_base, salt)
        );
    }
}
