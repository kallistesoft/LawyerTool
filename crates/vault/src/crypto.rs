//! Key derivation and key wrapping.
//!
//! - DEK: random 256-bit key that SQLCipher uses for the database.
//! - Passphrase KEK: Argon2id(passphrase, salt). Wraps the DEK in the keychain.
//! - Recovery KEK: HKDF-SHA256 of a random 256-bit recovery secret, shown to
//!   the user once as 24 words. Wraps the DEK in the key file next to the DB.
//!
//! Wrapping is AES-256-GCM with the vault id and purpose as associated data,
//! so a wrapped key cannot be moved to another vault or slot.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::Error;

pub const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SALT_LEN: usize = 16;

/// A 256-bit secret, wiped from memory when dropped.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Secret([u8; KEY_LEN]);

impl Secret {
    pub fn random() -> Result<Self, Error> {
        let mut k = [0u8; KEY_LEN];
        getrandom::fill(&mut k).map_err(|_| Error::Crypto("system random source unavailable"))?;
        Ok(Self(k))
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, Error> {
        let arr: [u8; KEY_LEN] = b.try_into().map_err(|_| Error::Corrupt("key length"))?;
        Ok(Self(arr))
    }

    pub fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// SQLCipher raw-key form: `x'<64 hex chars>'`.
    pub fn sqlcipher_literal(&self) -> Zeroizing<String> {
        let mut s = Zeroizing::new(String::with_capacity(67));
        s.push_str("x'");
        for b in self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s.push('\'');
        s
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

fn random<const N: usize>() -> Result<[u8; N], Error> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| Error::Crypto("system random source unavailable"))?;
    Ok(b)
}

/// Argon2id cost parameters, stored with the wrapped key so they can be
/// raised later without breaking existing vaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// Production default: 64 MiB, 3 passes (about half a second on a laptop).
    pub const DEFAULT: Self = Self {
        m_cost: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };
    /// Fast parameters for tests only.
    pub const INSECURE_FAST_FOR_TESTS: Self = Self {
        m_cost: 64,
        t_cost: 1,
        p_cost: 1,
    };
}

// OWASP minimum for Argon2id is 19 MiB / 2 passes; the default must stay above it.
const _: () = assert!(KdfParams::DEFAULT.m_cost >= 64 * 1024 && KdfParams::DEFAULT.t_cost >= 3);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wrapped {
    #[serde(with = "hex")]
    pub nonce: Vec<u8>,
    #[serde(with = "hex")]
    pub ciphertext: Vec<u8>,
}

/// What the keychain holds: the passphrase-wrapped DEK and how to derive the KEK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassphraseSlot {
    pub version: u32,
    pub kdf: KdfParams,
    #[serde(with = "hex")]
    pub salt: Vec<u8>,
    pub wrapped: Wrapped,
}

pub fn passphrase_kek(passphrase: &str, salt: &[u8], kdf: KdfParams) -> Result<Secret, Error> {
    let params = Params::new(kdf.m_cost, kdf.t_cost, kdf.p_cost, Some(KEY_LEN))
        .map_err(|_| Error::Crypto("invalid KDF parameters"))?;
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, out.as_mut())
        .map_err(|_| Error::Crypto("key derivation failed"))?;
    Secret::from_bytes(out.as_ref())
}

pub fn recovery_kek(recovery: &Secret, vault_id: &str) -> Result<Secret, Error> {
    let hk = Hkdf::<Sha256>::new(Some(vault_id.as_bytes()), recovery.expose());
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(b"lawyertool recovery kek v1", out.as_mut())
        .map_err(|_| Error::Crypto("key derivation failed"))?;
    Secret::from_bytes(out.as_ref())
}

/// Borrowed views: the key bytes are never copied into a non-zeroizing buffer.
fn key_ref(k: &Secret) -> Result<&Key<Aes256Gcm>, Error> {
    <&Key<Aes256Gcm>>::try_from(&k.expose()[..]).map_err(|_| Error::Crypto("key length"))
}

fn nonce_ref(n: &[u8]) -> Result<&Nonce<aes_gcm::aead::consts::U12>, Error> {
    <&Nonce<aes_gcm::aead::consts::U12>>::try_from(n).map_err(|_| Error::Crypto("nonce length"))
}

fn aad(vault_id: &str, slot: &str) -> Vec<u8> {
    format!("lawyertool-dek-v1|{vault_id}|{slot}").into_bytes()
}

pub fn wrap(kek: &Secret, dek: &Secret, vault_id: &str, slot: &str) -> Result<Wrapped, Error> {
    let cipher = Aes256Gcm::new(key_ref(kek)?);
    let nonce: [u8; NONCE_LEN] = random()?;
    let ciphertext = cipher
        .encrypt(
            nonce_ref(&nonce)?,
            Payload {
                msg: dek.expose(),
                aad: &aad(vault_id, slot),
            },
        )
        .map_err(|_| Error::Crypto("key wrapping failed"))?;
    Ok(Wrapped {
        nonce: nonce.to_vec(),
        ciphertext,
    })
}

/// `None` when the KEK is wrong (bad passphrase / phrase) or data was tampered with.
pub fn unwrap(kek: &Secret, w: &Wrapped, vault_id: &str, slot: &str) -> Option<Secret> {
    if w.nonce.len() != NONCE_LEN {
        return None;
    }
    let cipher = Aes256Gcm::new(key_ref(kek).ok()?);
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                nonce_ref(&w.nonce).ok()?,
                Payload {
                    msg: &w.ciphertext,
                    aad: &aad(vault_id, slot),
                },
            )
            .ok()?,
    );
    Secret::from_bytes(&plain).ok()
}

pub fn new_salt() -> Result<Vec<u8>, Error> {
    Ok(random::<SALT_LEN>()?.to_vec())
}

pub fn new_vault_id() -> Result<String, Error> {
    Ok(random::<16>()?.iter().map(|b| format!("{b:02x}")).collect())
}

/// Hex (de)serialization for byte vectors.
mod hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(b: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&b.iter().map(|x| format!("{x:02x}")).collect::<String>())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        if s.len() % 2 != 0 {
            return Err(serde::de::Error::custom("odd hex length"));
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(serde::de::Error::custom))
            .collect()
    }
}
