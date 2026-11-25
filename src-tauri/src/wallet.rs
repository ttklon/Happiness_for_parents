//! Wallet generation primitives: BIP39 (8 wordlists), BIP32 derivation,
//! Base58Check, Bech32/Bech32m, EVM/keccak addresses, Solana and TON.
//!
//! No external Tauri dependency here — this module is pure crypto so it can be
//! unit-tested independently.

use hmac::{Hmac, Mac};
use rand::RngCore;
use ripemd::Ripemd160;
use secp256k1::{PublicKey, Scalar, Secp256k1, SecretKey};
use sha2::{Digest, Sha256, Sha512};
use sha3::Keccak256;
use unicode_normalization::UnicodeNormalization;

const ENGLISH: &str = include_str!("bip39_english.txt");
const SPANISH: &str = include_str!("bip39_spanish.txt");
const FRENCH: &str = include_str!("bip39_french.txt");
const ITALIAN: &str = include_str!("bip39_italian.txt");
const JAPANESE: &str = include_str!("bip39_japanese.txt");
const KOREAN: &str = include_str!("bip39_korean.txt");
const CHINESE_S: &str = include_str!("bip39_chinese_simplified.txt");
const CHINESE_T: &str = include_str!("bip39_chinese_traditional.txt");

pub fn wordlist(language: &str) -> Vec<&'static str> {
    let raw = match language {
        "spanish" => SPANISH,
        "french" => FRENCH,
        "italian" => ITALIAN,
        "japanese" => JAPANESE,
        "korean" => KOREAN,
        "chinese_simplified" => CHINESE_S,
        "chinese_traditional" => CHINESE_T,
        _ => ENGLISH,
    };
    raw.lines().map(str::trim).filter(|s| !s.is_empty()).collect()
}

pub fn normalize(text: &str) -> String {
    text.nfkd().collect()
}

fn hmac512(key: &[u8], data: &[u8]) -> [u8; 64] {
    type HmacSha512 = Hmac<Sha512>;
    let mut mac = HmacSha512::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    out
}

fn sha256d(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    let mut out = [0u8; 32];
    out.copy_from_slice(&second);
    out
}

pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    let mut out = [0u8; 20];
    out.copy_from_slice(&ripe);
    out
}

pub fn hex_encode(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Base58Check / Base58 (implemented locally to avoid crate-version churn)
// ---------------------------------------------------------------------------

const B58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub fn base58_encode(input: &[u8]) -> String {
    let zeros = input.iter().take_while(|&&b| b == 0).count();
    let mut digits: Vec<u8> = vec![0];
    for &byte in input {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    // Remove leading zero digits produced by the big-integer loop, then restore
    // the canonical leading '1's for zero bytes.
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    let mut out = String::with_capacity(input.len() * 2);
    for _ in 0..zeros {
        out.push('1' as char);
    }
    for &d in digits.iter().rev() {
        out.push(B58[d as usize] as char);
    }
    out
}

fn base58_check(version: u8, payload: &[u8]) -> String {
    let mut data = Vec::with_capacity(payload.len() + 21);
    data.push(version);
    data.extend_from_slice(payload);
    let checksum = sha256d(&data);
    data.extend_from_slice(&checksum[..4]);
    base58_encode(&data)
}

// ---------------------------------------------------------------------------
// Bech32 / Bech32m
// ---------------------------------------------------------------------------

fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();
    let maxv = (1 << to) - 1;
    for &value in data {
        if (value as u32) >> from != 0 {
            return None;
        }
        acc = (acc << from) | value as u32;
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad {
        if bits > 0 {
            out.push(((acc << (to - bits)) & maxv) as u8);
        }
    } else if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return None;
    }
    Some(out)
}

fn polymod(values: &[u8]) -> u32 {
    const GEN: [u32; 5] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
    let mut chk = 1u32;
    for &v in values {
        let top = chk >> 25;
        chk = ((chk & 0x1ffffff) << 5) ^ v as u32;
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn bech32_hrp_expand(hrp: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hrp.len() * 2 + 1);
    out.extend(hrp.bytes().map(|b| b >> 5));
    out.push(0);
    out.extend(hrp.bytes().map(|b| b & 31));
    out
}

fn bech32_encode(hrp: &str, witness_version: u8, program: &[u8]) -> String {
    let mut data = vec![witness_version];
    data.extend(convert_bits(program, 8, 5, true).expect("valid witness program"));
    let bech32m = witness_version > 0;
    let constant = if bech32m { 0x2bc830a3 } else { 1 };
    let mut values = bech32_hrp_expand(hrp);
    values.extend_from_slice(&data);
    values.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let checksum = polymod(&values) ^ constant;
    for i in 0..6 {
        data.push(((checksum >> (5 * (5 - i))) & 31) as u8);
    }
    let charset = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    let mut out = String::with_capacity(hrp.len() + 1 + data.len());
    out.push_str(hrp);
    out.push('1');
    for d in data {
        out.push(charset[d as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// BIP39
// ---------------------------------------------------------------------------

pub fn mnemonic_from_entropy(entropy: &[u8], language: &str) -> String {
    let list = wordlist(language);
    let checksum_bits = (entropy.len() * 8) / 32;
    let hash = Sha256::digest(entropy);
    let mut bits: Vec<bool> = Vec::with_capacity(entropy.len() * 8 + checksum_bits);
    for &b in entropy {
        for i in (0..8).rev() {
            bits.push((b >> i) & 1 == 1);
        }
    }
    for i in 0..checksum_bits {
        bits.push((hash[i / 8] >> (7 - (i % 8))) & 1 == 1);
    }
    let mut phrase = Vec::with_capacity(bits.len() / 11);
    for chunk in bits.chunks(11) {
        let mut index = 0usize;
        for &bit in chunk {
            index = (index << 1) | usize::from(bit);
        }
        phrase.push(list[index % list.len()]);
    }
    phrase.join(" ")
}

pub fn random_mnemonic<R: RngCore>(rng: &mut R, language: &str, words: usize) -> String {
    let entropy_len = if words >= 24 { 32 } else { 16 };
    let mut entropy = vec![0u8; entropy_len];
    rng.fill_bytes(&mut entropy);
    mnemonic_from_entropy(&entropy, language)
}

/// BIP39 seed (PBKDF2-HMAC-SHA512, 2048 rounds).
pub fn mnemonic_to_seed(phrase: &str, passphrase: &str) -> [u8; 64] {
    let phrase = normalize(phrase);
    let salt = normalize(&format!("mnemonic{passphrase}"));
    let mut seed = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<Sha512>(phrase.as_bytes(), salt.as_bytes(), 2048, &mut seed);
    seed
}

/// Brain wallet key: SHA256 of a normalized phrase.
pub fn brain_key(phrase: &str) -> [u8; 32] {
    let normalized = normalize(phrase);
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha256::digest(normalized.as_bytes()));
    out
}

pub fn valid_secret(bytes: &[u8]) -> [u8; 32] {
    if let Ok(key) = SecretKey::from_slice(bytes) {
        return key.secret_bytes();
    }
    let mut candidate = [0u8; 32];
    candidate.copy_from_slice(bytes);
    let mut counter = 0u8;
    while SecretKey::from_slice(&candidate).is_err() {
        counter = counter.wrapping_add(1);
        let mut input = candidate.to_vec();
        input.push(counter);
        let digest = Sha256::digest(input);
        candidate.copy_from_slice(&digest);
    }
    candidate
}

// ---------------------------------------------------------------------------
// BIP32 (private derivation; hardened and normal paths)
// ---------------------------------------------------------------------------

pub fn derive_bip32(seed: &[u8], path: &[u32]) -> Result<[u8; 32], String> {
    let master = hmac512(b"Bitcoin seed", seed);
    let mut key = SecretKey::from_slice(&master[..32])
        .map_err(|_| "invalid BIP32 master key".to_string())?;
    let mut chain = [0u8; 32];
    chain.copy_from_slice(&master[32..]);

    for &index in path {
        let secp = Secp256k1::new();
        let mut data = Vec::with_capacity(37);
        if index >= 0x8000_0000 {
            data.push(0u8);
            data.extend_from_slice(&key.secret_bytes());
        } else {
            let public = PublicKey::from_secret_key(&secp, &key);
            data.extend_from_slice(&public.serialize());
        }
        data.extend_from_slice(&index.to_be_bytes());

        let child = hmac512(&chain, &data);
        let tweak = Scalar::from_be_bytes({
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(&child[..32]);
            bytes
        })
        .map_err(|_| "BIP32 child scalar out of range".to_string())?;
        key = key
            .add_tweak(&tweak)
            .map_err(|_| "BIP32 child key derivation failed".to_string())?;
        chain.copy_from_slice(&child[32..]);
    }
    Ok(key.secret_bytes())
}

// ---------------------------------------------------------------------------
// Coin families and address encoding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Utxo,
    Evm,
    Tron,
    Solana,
    Ton,
}

#[derive(Debug, Clone, Copy)]
pub struct CoinSpec {
    pub ticker: &'static str,
    pub name: &'static str,
    pub coin_type: u32,
    pub family: Family,
    pub p2pkh: u8,
    pub p2sh: u8,
    pub hrp: &'static str,
}

pub fn coin_spec(ticker: &str) -> CoinSpec {
    match ticker {
        "LTC" => CoinSpec { ticker: "LTC", name: "Litecoin", coin_type: 2, family: Family::Utxo, p2pkh: 0x30, p2sh: 0x32, hrp: "ltc" },
        "DOGE" => CoinSpec { ticker: "DOGE", name: "Dogecoin", coin_type: 3, family: Family::Utxo, p2pkh: 0x1e, p2sh: 0x16, hrp: "doge" },
        "BCH" => CoinSpec { ticker: "BCH", name: "Bitcoin Cash", coin_type: 145, family: Family::Utxo, p2pkh: 0x00, p2sh: 0x05, hrp: "bc" },
        "ETH" => CoinSpec { ticker: "ETH", name: "Ethereum", coin_type: 60, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "MATIC" => CoinSpec { ticker: "MATIC", name: "Polygon", coin_type: 966, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "BSC" => CoinSpec { ticker: "BSC", name: "BNB Smart Chain", coin_type: 714, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "ARB" => CoinSpec { ticker: "ARB", name: "Arbitrum", coin_type: 118421, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "OPT" => CoinSpec { ticker: "OPT", name: "Optimism", coin_type: 614, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "BASE" => CoinSpec { ticker: "BASE", name: "Base", coin_type: 8453, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "AVAX" => CoinSpec { ticker: "AVAX", name: "Avalanche C-Chain", coin_type: 9005, family: Family::Evm, p2pkh: 0, p2sh: 0, hrp: "" },
        "SOL" => CoinSpec { ticker: "SOL", name: "Solana", coin_type: 501, family: Family::Solana, p2pkh: 0, p2sh: 0, hrp: "" },
        "TON" => CoinSpec { ticker: "TON", name: "Toncoin", coin_type: 607, family: Family::Ton, p2pkh: 0, p2sh: 0, hrp: "" },
        _ => CoinSpec { ticker: "BTC", name: "Bitcoin", coin_type: 0, family: Family::Utxo, p2pkh: 0x00, p2sh: 0x05, hrp: "bc" },
    }
}

/// Purpose for the requested address type (BIP44/49/84/86).
pub fn purpose(address_type: &str) -> u32 {
    match address_type {
        "segwit" => 49,
        "native" => 84,
        "taproot" => 86,
        _ => 44,
    }
}

pub fn derivation_path(coin: &str, address_type: &str) -> Vec<u32> {
    let spec = coin_spec(coin);
    vec![
        0x8000_0000 | purpose(address_type),
        0x8000_0000 | spec.coin_type,
        0x8000_0000,
        0,
        0,
    ]
}

fn keccak_pubkey(pubkey: &PublicKey) -> [u8; 20] {
    let uncompressed = pubkey.serialize_uncompressed();
    let hash = Keccak256::digest(&uncompressed[1..]);
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[hash.len() - 20..]);
    out
}

fn ed25519_public(secret: &[u8; 32]) -> [u8; 32] {
    use ed25519_dalek::SigningKey;
    let signing = SigningKey::from_bytes(secret);
    signing.verifying_key().to_bytes()
}

fn ton_address(secret: &[u8; 32]) -> String {
    const CRC16_CCITT: u16 = 0x1021;
    let public = ed25519_public(secret);
    let account = Sha256::digest(public);
    let mut payload = Vec::with_capacity(36);
    payload.push(0x11); // bounceable
    payload.push(0x00); // workchain 0
    payload.extend_from_slice(&account[..32]);
    let mut crc = 0u16;
    for &b in &payload {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ CRC16_CCITT
            } else {
                crc << 1
            };
        }
    }
    payload.extend_from_slice(&crc.to_be_bytes());
    use base64::{engine::general_purpose::URL_SAFE, Engine};
    URL_SAFE.encode(payload)
}

/// Turn a 32-byte private key into a coin-specific address.
pub fn secret_to_address(secret: &[u8; 32], coin: &str, address_type: &str) -> Result<String, String> {
    let spec = coin_spec(coin);
    if spec.family == Family::Solana {
        return Ok(base58_encode(&ed25519_public(secret)));
    }
    if spec.family == Family::Ton {
        return Ok(ton_address(secret));
    }

    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(secret).map_err(|_| "invalid secret key".to_string())?;
    let public = PublicKey::from_secret_key(&secp, &key);

    match spec.family {
        Family::Evm => Ok(format!("0x{}", hex_encode(&keccak_pubkey(&public)))),
        Family::Tron => {
            let digest = keccak_pubkey(&public);
            Ok(base58_check(0x41, &digest))
        }
        Family::Utxo => {
            let compressed = public.serialize();
            let hash = hash160(&compressed);
            match address_type {
                "legacy" => Ok(base58_check(spec.p2pkh, &hash)),
                "segwit" => {
                    let mut redeem = vec![0x00, 0x14];
                    redeem.extend_from_slice(&hash);
                    let script_hash = hash160(&redeem);
                    Ok(base58_check(spec.p2sh, &script_hash))
                }
                "native" => Ok(bech32_encode(spec.hrp, 0, &hash)),
                "taproot" => Ok(bech32_encode(spec.hrp, 1, &public.serialize()[1..])),
                // "auto": try all four formats; the caller can rotate formats
                // by passing an explicit type. Legacy is the safe default.
                _ => Ok(base58_check(spec.p2pkh, &hash)),
            }
        }
        Family::Solana | Family::Ton => unreachable!(),
    }
}

/// Derive a usable key from a candidate phrase/entropy for a particular coin.
pub fn key_for_coin(
    raw_secret: &[u8; 32],
    seed: Option<&[u8; 64]>,
    coin: &str,
    address_type: &str,
) -> [u8; 32] {
    let spec = coin_spec(coin);
    match seed {
        Some(seed) if spec.family != Family::Solana && spec.family != Family::Ton => {
            derive_bip32(seed, &derivation_path(coin, address_type)).unwrap_or_else(|_| *raw_secret)
        }
        Some(seed) if spec.family == Family::Solana || spec.family == Family::Ton => {
            // SLIP-0010 ed25519 master key: HMAC-SHA512("ed25519 seed", seed).
            let master = hmac512(b"ed25519 seed", seed);
            let mut out = [0u8; 32];
            out.copy_from_slice(&master[..32]);
            out
        }
        _ => valid_secret(raw_secret),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bip39_vector() {
        let entropy = vec![0u8; 16];
        let phrase = mnemonic_from_entropy(&entropy, "english");
        assert_eq!(
            phrase,
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        );
        let seed = mnemonic_to_seed(&phrase, "TREZOR");
        assert_eq!(
            hex_encode(&seed[..4]),
            "c55257c3",
            "BIP39 test vector seed must match"
        );
    }

    #[test]
    fn known_bip32_vector() {
        // BIP32 test vector 1: m/0' from seed 000102030405060708090a0b0c0d0e0f.
        let seed: Vec<u8> = (0u8..16).collect();
        let key = derive_bip32(&seed, &[0x8000_0000]).unwrap();
        assert_eq!(
            hex_encode(&key),
            "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea"
        );
    }

    #[test]
    fn known_base58_and_bech32() {
        // Compressed pubkey of private key 1 → classic BTC P2PKH address.
        let secret = [0u8; 31].into_iter().chain([1u8]).collect::<Vec<_>>();
        let mut key = [0u8; 32];
        key.copy_from_slice(&secret);
        let addr = secret_to_address(&key, "BTC", "legacy").unwrap();
        assert_eq!(addr, "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
    }
}
