//! ADB RSA key management: generation, persistence (`~/.android/adbkey`,
//! compatible with the real `adb` client/daemon), signing, and the
//! Android-specific public key wire format used by `A_AUTH` `RSAPUBLICKEY`
//! packets.
//!
//! Wire format reference: `system/core/libcrypto_utils/android_pubkey.cpp`
//! (AOSP) - a fixed-size 2048-bit-only binary struct, base64-encoded and
//! suffixed with `" user@host"`.

use android_boot_protocol::adb::AdbAuthSigner;
use android_boot_protocol::error::{Error, Result};
use base64::Engine;
use num_bigint::BigUint;
use rand_core::{TryCryptoRng, TryRng};
use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey};
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey};
use sha1::Sha1;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

/// RSA modulus size (bits) used for ADB authentication keys. The Android
/// public-key wire format (`android_pubkey.cpp`) hard-codes this size.
const RSA_KEY_BITS: usize = 2048;
const MODULUS_BYTES: usize = RSA_KEY_BITS / 8;
const MODULUS_WORDS: u32 = (RSA_KEY_BITS / 32) as u32;

/// A CSPRNG backed by the OS random source (`getrandom`), bridged into the
/// `rand_core` 0.10 traits expected by the `rsa` crate.
struct SystemRng;

impl TryRng for SystemRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> core::result::Result<u32, Self::Error> {
        let mut buf = [0u8; 4];
        getrandom::fill(&mut buf).expect("system RNG failure");
        Ok(u32::from_ne_bytes(buf))
    }

    fn try_next_u64(&mut self) -> core::result::Result<u64, Self::Error> {
        let mut buf = [0u8; 8];
        getrandom::fill(&mut buf).expect("system RNG failure");
        Ok(u64::from_ne_bytes(buf))
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> core::result::Result<(), Self::Error> {
        getrandom::fill(dst).expect("system RNG failure");
        Ok(())
    }
}

impl TryCryptoRng for SystemRng {}

/// A persisted (or freshly generated) ADB RSA keypair, plus a precomputed
/// device-registration blob for `A_AUTH` `RSAPUBLICKEY` packets.
pub struct AdbKey {
    private_key: RsaPrivateKey,
    /// Base64-encoded Android public key struct + `" user@host"` suffix
    /// (no trailing NUL - `AdbClient` appends that when framing the packet).
    public_key_line: String,
}

impl AdbKey {
    /// Loads the key at `path`, generating (and persisting) a new one if it
    /// doesn't exist yet - mirroring `adb`'s own `~/.android/adbkey` /
    /// `adbkey.pub` behavior, including file format, so keys are
    /// interchangeable with a real `adb` install.
    pub fn load_or_generate(path: &Path) -> Result<Self> {
        let private_key = if path.exists() {
            let pem = fs::read_to_string(path)
                .map_err(|e| Error::Message(format!("failed to read {}: {e}", path.display())))?;
            // Real `adb` has historically written PKCS#1 ("RSA PRIVATE KEY")
            // keys, but modern versions (and other tools) write PKCS#8
            // ("PRIVATE KEY"). Accept either for interoperability.
            RsaPrivateKey::from_pkcs1_pem(&pem)
                .or_else(|_| RsaPrivateKey::from_pkcs8_pem(&pem))
                .map_err(|_| {
                    Error::Custom("failed to parse adbkey PEM (expected PKCS#1 or PKCS#8)")
                })?
        } else {
            let mut rng = SystemRng;
            let key = RsaPrivateKey::new(&mut rng, RSA_KEY_BITS)
                .map_err(|_| Error::Custom("RSA key generation failed"))?;
            Self::persist(path, &key)?;
            key
        };

        let public_key = RsaPublicKey::from(&private_key);
        let encoded = android_pubkey_encode(&public_key)?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(encoded);
        let public_key_line = format!("{b64} {}", user_at_host());

        Ok(Self {
            private_key,
            public_key_line,
        })
    }

    fn persist(path: &Path, key: &RsaPrivateKey) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::Message(format!("failed to create {}: {e}", parent.display()))
            })?;
        }

        let pem = key
            .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .map_err(|_| Error::Custom("failed to encode private key as PKCS#1 PEM"))?;
        fs::write(path, pem.as_bytes())
            .map_err(|e| Error::Message(format!("failed to write {}: {e}", path.display())))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }

        let public_key = RsaPublicKey::from(key);
        let encoded = android_pubkey_encode(&public_key)?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(encoded);
        let pub_line = format!("{b64} {}\n", user_at_host());
        let pub_path = PathBuf::from(format!("{}.pub", path.display()));
        let _ = fs::write(pub_path, pub_line);

        Ok(())
    }
}

impl AdbAuthSigner for AdbKey {
    fn sign(&self, token: &[u8]) -> Result<Option<Vec<u8>>> {
        // Matches adbd's `RSA_sign(NID_sha1, token, ...)`: PKCS#1 v1.5
        // padding with the SHA1 DigestInfo prefix, applied directly to the
        // device's random token (which is *treated* as a SHA1 digest, not
        // actually hashed again here).
        let scheme = Pkcs1v15Sign::new::<Sha1>();
        let sig = self
            .private_key
            .sign(scheme, token)
            .map_err(|_| Error::Custom("RSA signing failed"))?;
        Ok(Some(sig))
    }

    fn public_key(&self) -> Option<&[u8]> {
        Some(self.public_key_line.as_bytes())
    }
}

/// Returns the default `adbkey` path (`~/.android/adbkey`), matching the
/// real `adb` client's `ANDROID_SDK_HOME`/`HOME` fallback logic (simplified
/// to just `HOME`).
pub fn default_key_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".android").join("adbkey")
}

fn user_at_host() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    format!("{user}@{host}")
}

/// Encodes an RSA public key using Android's fixed-size (2048-bit-only)
/// binary struct, as implemented by `android_pubkey_encode` in AOSP's
/// `libcrypto_utils`:
///
/// ```text
/// struct RSAPublicKey {
///     uint32_t modulus_size_words; // = 64 for 2048-bit keys
///     uint32_t n0inv;              // -1 / n[0] mod 2^32
///     uint8_t modulus[256];        // little-endian
///     uint8_t rr[256];             // R^2 mod n, little-endian, R = 2^2048
///     uint32_t exponent;           // e.g. 65537
/// }
/// ```
fn android_pubkey_encode(pub_key: &RsaPublicKey) -> Result<Vec<u8>> {
    if pub_key.size() != MODULUS_BYTES {
        return Err(Error::Custom(
            "Android ADB public key format only supports 2048-bit RSA keys",
        ));
    }

    let n_bytes_be = pub_key.n_bytes();
    let e_bytes_be = pub_key.e_bytes();
    let n_big = BigUint::from_bytes_be(&n_bytes_be);
    let e_big = BigUint::from_bytes_be(&e_bytes_be);

    // n0inv = -1 / n[0] mod 2^32, computed purely in u64 arithmetic since
    // the modulus (2^32) is small - avoids needing bignum modular inverse.
    let r32: u64 = 1u64 << 32;
    let n0: u64 = {
        let n_le = n_big.to_bytes_le();
        let mut buf = [0u8; 8];
        let len = n_le.len().min(4);
        buf[..len].copy_from_slice(&n_le[..len]);
        u64::from_le_bytes(buf) & 0xFFFF_FFFF
    };
    let inv = mod_inverse_u64(n0, r32).ok_or(Error::Custom("RSA modulus is not odd"))?;
    let n0inv = (r32 - inv) as u32;

    // modulus, little-endian, fixed width
    let mut modulus = n_big.to_bytes_le();
    modulus.resize(MODULUS_BYTES, 0);

    // rr = (2^(MODULUS_BYTES*8))^2 mod n = 2^(2*RSA_KEY_BITS) mod n
    let exponent_bits = BigUint::from(2u32 * RSA_KEY_BITS as u32);
    let two = BigUint::from(2u32);
    let rr_big = two.modpow(&exponent_bits, &n_big);
    let mut rr = rr_big.to_bytes_le();
    rr.resize(MODULUS_BYTES, 0);

    let exponent: u32 = e_big.to_u32_digits().first().copied().unwrap_or(0);

    let mut out = Vec::with_capacity(4 + 4 + MODULUS_BYTES + MODULUS_BYTES + 4);
    out.extend_from_slice(&MODULUS_WORDS.to_le_bytes());
    out.extend_from_slice(&n0inv.to_le_bytes());
    out.extend_from_slice(&modulus);
    out.extend_from_slice(&rr);
    out.extend_from_slice(&exponent.to_le_bytes());

    Ok(out)
}

/// Computes the modular multiplicative inverse of `a` mod `m` (m = 2^32
/// here) via the extended Euclidean algorithm, using `i128` to avoid
/// overflow. Returns `None` if `a` is not invertible mod `m` (i.e. even,
/// since `m` is a power of two).
fn mod_inverse_u64(a: u64, m: u64) -> Option<u64> {
    let (mut old_r, mut r) = (a as i128, m as i128);
    let (mut old_s, mut s) = (1i128, 0i128);

    while r != 0 {
        let q = old_r / r;
        let tmp_r = old_r - q * r;
        old_r = r;
        r = tmp_r;
        let tmp_s = old_s - q * s;
        old_s = s;
        s = tmp_s;
    }

    if old_r != 1 {
        return None;
    }

    let m = m as i128;
    let result = ((old_s % m) + m) % m;
    Some(result as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keygen_and_sign_roundtrip() {
        let mut rng = SystemRng;
        let key = RsaPrivateKey::new(&mut rng, RSA_KEY_BITS).expect("keygen failed");
        let pubkey = RsaPublicKey::from(&key);

        // Sign a fake 20-byte SHA1 "token" like the device would send.
        let token = [0x42u8; 20];
        let scheme = Pkcs1v15Sign::new::<Sha1>();
        let sig = key.sign(scheme, &token).expect("sign failed");
        assert_eq!(sig.len(), RSA_KEY_BITS / 8);

        // Verify using the public key with the same scheme.
        let verify_scheme = Pkcs1v15Sign::new::<Sha1>();
        pubkey
            .verify(verify_scheme, &token, &sig)
            .expect("signature verification failed");
    }

    #[test]
    fn test_android_pubkey_encode_format() {
        let mut rng = SystemRng;
        let key = RsaPrivateKey::new(&mut rng, RSA_KEY_BITS).expect("keygen failed");
        let pubkey = RsaPublicKey::from(&key);

        let encoded = android_pubkey_encode(&pubkey).expect("encode failed");
        // 3 * u32 + 2 * 256 bytes = 524 bytes total.
        assert_eq!(encoded.len(), 524);

        let modulus_size_words = u32::from_le_bytes(encoded[0..4].try_into().unwrap());
        assert_eq!(modulus_size_words, 64);

        let exponent = u32::from_le_bytes(encoded[520..524].try_into().unwrap());
        assert_eq!(exponent, 65537);
    }

    #[test]
    fn test_mod_inverse() {
        // 3 * inv(3) mod 2^32 == 1
        let inv = mod_inverse_u64(3, 1u64 << 32).unwrap();
        assert_eq!((3u64.wrapping_mul(inv)) & 0xFFFF_FFFF, 1);

        // Even numbers aren't invertible mod a power of two.
        assert_eq!(mod_inverse_u64(4, 1u64 << 32), None);
    }
}
